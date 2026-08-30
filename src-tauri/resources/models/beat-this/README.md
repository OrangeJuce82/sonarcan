The release build bundles Beat This! `final0.ckpt` from the official CPJKU
distribution. Run `npm run chords:downbeat-model` to download it. The preparation
script verifies SHA-256 before accepting the file; the production worker verifies
the same digest again before loading the model. Beat This! code and published
weights are distributed under the MIT license.
The bundled license text is retained in `LICENSE-MIT.txt`.
