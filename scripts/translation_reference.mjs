// Reference translations from transformers.js for crates/boltay-translate/tests/fixtures.
//
// Node resolves the package next to the script, so run a copy placed beside node_modules:
//   npm install --ignore-scripts @huggingface/transformers
//   cp scripts/translation_reference.mjs . && node translation_reference.mjs models/opus-mt-ru-en < phrases.txt
//
// --ignore-scripts skips the GPU binaries onnxruntime-node fetches on install, the CPU
// runtime ships in the package.
//
// One phrase per input line, output is `phrase<TAB>translation`. Greedy decoding with the
// same repetition guards as boltay-translate; the model's generation config asks for 6 beams.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { env, pipeline } from "@huggingface/transformers";

const dir = path.resolve(process.argv[2]);

// transformers.js expects the ONNX files in an onnx/ subdirectory.
const root = fs.mkdtempSync(path.join(os.tmpdir(), "opus-mt-"));
const model = path.join(root, "opus-mt-ru-en");
fs.mkdirSync(path.join(model, "onnx"), { recursive: true });
for (const f of ["config.json", "generation_config.json", "tokenizer.json", "tokenizer_config.json"]) {
  fs.symlinkSync(path.join(dir, f), path.join(model, f));
}
for (const f of ["encoder_model_quantized.onnx", "decoder_model_merged_quantized.onnx"]) {
  fs.symlinkSync(path.join(dir, f), path.join(model, "onnx", f));
}

env.localModelPath = root;
env.allowRemoteModels = false;
const translate = await pipeline("translation", "opus-mt-ru-en", { dtype: "q8", device: "cpu" });

const lines = fs.readFileSync(0, "utf8").split("\n").filter((l) => l.trim());
for (const line of lines) {
  // Same sentence split as boltay-translate.
  const parts = [];
  for (const sentence of line.split(/(?<=[.!?…])\s+/).map((s) => s.trim()).filter(Boolean)) {
    const [out] = await translate(sentence, {
      num_beams: 1,
      do_sample: false,
      no_repeat_ngram_size: 3,
      repetition_penalty: 1.05,
    });
    parts.push(out.translation_text);
  }
  console.log(`${line}\t${parts.join(" ")}`);
}
fs.rmSync(root, { recursive: true });
