#!/usr/bin/env python3
# Reference transcripts from onnx-asr for crates/boltay-core/tests/fixtures.
#
#   pip install onnx-asr onnxruntime==1.28.0
#   scripts/reference.py models crates/boltay-core/tests/fixtures/{gigaam_example,tts_ru_?}.wav
#   scripts/reference.py models --model parakeet crates/boltay-core/tests/fixtures/{jfk,tts_en_?,tts_uk_?}.wav
#   scripts/reference.py models --gap 1 a.wav b.wav
#
# onnxruntime must match the version boltay links: the int8 encoder can flip
# a token on borderline audio between runtime versions.
import argparse
from pathlib import Path

import numpy as np
import onnx_asr
from onnx_asr.utils import read_wav

SR = 16_000
MAX_SINGLE_PASS = 25 * SR
MAX_CHUNK = 20 * SR
MODELS = {
    "gigaam": ("gigaam-v3-e2e-rnnt", "gigaam-v3-e2e-rnnt"),
    "parakeet": ("nemo-parakeet-tdt-0.6b-v3", "parakeet-v3"),
}


def chunks(segments):
    # Same grouping as boltay_core::Recognizer.
    if not segments:
        return []
    limit = MAX_SINGLE_PASS if segments[-1][1] - segments[0][0] <= MAX_SINGLE_PASS else MAX_CHUNK
    out, (start, end) = [], segments[0]
    for s, e in segments[1:]:
        if e - start <= limit:
            end = e
        else:
            out.append((start, end))
            start, end = s, e
    return out + [(start, end)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("models", type=Path)
    parser.add_argument("files", nargs="+")
    parser.add_argument("--model", choices=MODELS, default="gigaam")
    parser.add_argument("--gap", type=float, help="join the files with this much silence (s) and run through VAD")
    args = parser.parse_args()

    name, subdir = MODELS[args.model]
    asr = onnx_asr.load_model(name, args.models / subdir, quantization="int8")
    if args.gap is None:
        for f in args.files:
            print(f"{Path(f).name}\t{asr.recognize(f)}")
        return

    silence = np.zeros(int(args.gap * SR), dtype=np.float32)
    parts = []
    for f in args.files:
        wave, sr = read_wav(f)
        assert sr == SR, f
        parts += [wave[:, 0], silence]
    audio = np.concatenate(parts[:-1])

    vad = onnx_asr.load_vad("silero", args.models)
    segments = list(next(vad.segment_batch(audio[None], np.array([len(audio)]), SR)))
    texts = [asr.recognize(audio[s:e]) for s, e in chunks(segments)]
    print(" ".join(t for t in texts if t))


if __name__ == "__main__":
    main()
