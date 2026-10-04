## Speech and translation models

The models are not part of the installer. The app downloads them on first use from the
`models-v1` release of the Boltay repository, or from Hugging Face when that fails. The
files are the ONNX exports listed in the second table, unchanged.

| Model | Used for | Authors | License |
| --- | --- | --- | --- |
| [GigaAM v3](https://github.com/salute-developers/GigaAM) | Russian speech | SberDevices | MIT |
| [Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) | Speech in other languages | NVIDIA | CC-BY-4.0 |
| [Silero VAD](https://github.com/snakers4/silero-vad) | Telling speech from silence | Silero Team | MIT |
| [opus-mt-ru-en](https://huggingface.co/Helsinki-NLP/opus-mt-ru-en) | Russian to English | Helsinki-NLP | CC-BY-4.0 |
| [opus-mt-tc-big-zle-en](https://huggingface.co/Helsinki-NLP/opus-mt-tc-big-zle-en) | Russian to English, quality | Helsinki-NLP | CC-BY-4.0 |
| [opus-mt-en-ru](https://huggingface.co/Helsinki-NLP/opus-mt-en-ru) | English to Russian | Helsinki-NLP | Apache-2.0 |
| [opus-mt-tc-big-en-zle](https://huggingface.co/Helsinki-NLP/opus-mt-tc-big-en-zle) | English to Russian, quality | Helsinki-NLP | CC-BY-4.0 |

| ONNX export | Of | License |
| --- | --- | --- |
| [istupakov/gigaam-v3-onnx](https://huggingface.co/istupakov/gigaam-v3-onnx) | GigaAM v3, int8 | MIT |
| [istupakov/parakeet-tdt-0.6b-v3-onnx](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx) | Parakeet TDT 0.6B v3, int8 | CC-BY-4.0 |
| [istupakov/silero-vad-onnx](https://huggingface.co/istupakov/silero-vad-onnx) | Silero VAD | MIT |
| [Xenova/opus-mt-ru-en](https://huggingface.co/Xenova/opus-mt-ru-en) | opus-mt-ru-en, quantized | CC-BY-4.0, the model's own |
| [TigreGotico/opus-mt-tc-big-zle-en-onnx](https://huggingface.co/TigreGotico/opus-mt-tc-big-zle-en-onnx) | opus-mt-tc-big-zle-en, int8 | CC-BY-4.0 |
| [Xenova/opus-mt-en-ru](https://huggingface.co/Xenova/opus-mt-en-ru) | opus-mt-en-ru, quantized | Apache-2.0, the model's own |
| [TigreGotico/opus-mt-tc-big-en-zle-onnx](https://huggingface.co/TigreGotico/opus-mt-tc-big-en-zle-onnx) | opus-mt-tc-big-en-zle, int8 | CC-BY-4.0 |

The exports are conversions of the original models to ONNX, most of them quantized to
8 bits. Neither the authors of the models nor those of the exports endorse Boltay.
CC-BY-4.0: https://creativecommons.org/licenses/by/4.0/,
Apache-2.0: https://www.apache.org/licenses/LICENSE-2.0.

