#!/usr/bin/env python3
# Reference translations for models that ship their SentencePiece files (opus-mt-tc-big-zle-en,
# opus-mt-en-ru): the official MarianTokenizer and a greedy loop over the ONNX decoders, split
# or merged, with the same repetition guards as boltay-translate. Output is
# `phrase<TAB>translation`.
#
#   pip install onnxruntime==1.28.0 transformers sentencepiece
#   scripts/translation_reference.py models/opus-mt-tc-big-zle-en < phrases.txt
#
# A multilingual model takes the target language token as the second argument:
#   scripts/translation_reference.py models/opus-mt-tc-big-en-zle '>>rus<<' < phrases.txt
import os, sys, re, json, numpy as np, onnxruntime as rt
from transformers import MarianTokenizer
d = sys.argv[1].rstrip('/') + '/'
target = sys.argv[2] + ' ' if len(sys.argv) > 2 else ''
tok = MarianTokenizer.from_pretrained(d)
gen = json.load(open(d + 'generation_config.json'))
o = rt.SessionOptions(); o.intra_op_num_threads = 4
merged = os.path.exists(d + 'decoder_model_merged_quantized.onnx')
if merged:
    enc = rt.InferenceSession(d + 'encoder_model_quantized.onnx', o)
    first = past_s = rt.InferenceSession(d + 'decoder_model_merged_quantized.onnx', o)
else:
    enc = rt.InferenceSession(d + 'encoder_model.onnx', o)
    first = rt.InferenceSession(d + 'decoder_model.onnx', o)
    past_s = rt.InferenceSession(d + 'decoder_with_past_model.onnx', o)

def empty_past(s):
    out = {}
    for i in s.get_inputs():
        if i.name.startswith('past_key_values'):
            out[i.name] = np.zeros((1, i.shape[1], 0, i.shape[3]), np.float32)
    return out

def translate(sentence):
    ids = tok(target + sentence).input_ids
    x = np.array([ids], np.int64); m = np.ones_like(x)
    h = enc.run(None, {'input_ids': x, 'attention_mask': m})[0]
    seq = [gen['decoder_start_token_id']]; past = {}
    for step in range(1, gen['max_length']):
        feed = {'encoder_attention_mask': m, 'input_ids': np.array([[seq[-1]]], np.int64)}
        s = first if step == 1 else past_s
        if step == 1 or merged: feed['encoder_hidden_states'] = h
        if merged:
            feed['use_cache_branch'] = np.array([step > 1])
            feed.update(empty_past(s) if step == 1 else past)
        elif step > 1:
            feed.update(past)
        names = [o.name for o in s.get_outputs()]
        r = s.run(None, feed)
        lg = r[0][0, -1].astype(np.float32).copy()
        for t in set(seq[1:]):
            lg[t] = lg[t] / 1.05 if lg[t] > 0 else lg[t] * 1.05
        banned = [w[0] for w in gen['bad_words_ids']]
        if len(seq) >= 2:
            pre = seq[-2:]
            banned += [seq[i + 2] for i in range(len(seq) - 2) if seq[i:i + 2] == pre]
        lg[banned] = -np.inf
        t = gen['eos_token_id'] if step + 1 == gen['max_length'] else int(lg.argmax())
        for n, v in zip(names[1:], r[1:]):
            if 'encoder' in n and step > 1: continue
            past[n.replace('present', 'past_key_values')] = v
        if t == gen['eos_token_id']: break
        seq.append(t)
    return tok.decode(seq[1:], skip_special_tokens=True)

for line in sys.stdin:
    line = line.strip()
    if not line: continue
    parts = [translate(s) for s in re.split(r'(?<=[.!?…])\s+', line) if s.strip()]
    print(line + '\t' + ' '.join(parts))
