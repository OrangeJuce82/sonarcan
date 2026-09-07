The release build uses Beat This! `final0.ckpt` from the official CPJKU
distribution as a build-time verification fixture. It is not bundled: the application
downloads the pinned file during first-run model setup. Run `npm run chords:downbeat-model` to prepare the fixture. The preparation
script verifies SHA-256 before accepting the file; the production worker verifies
the same digest again before loading the model. Beat This! code and published
weights are distributed under the MIT license.
The license text is retained in `LICENSE-MIT.txt`.
