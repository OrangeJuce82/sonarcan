The ExecuTorch export workflow uses Beat This! `final0.ckpt` from the official
CPJKU distribution as a development-only fixture. It is never bundled or loaded
by the application. Run `npm run chords:downbeat-model` to prepare the fixture;
the preparation script verifies its pinned SHA-256 before accepting it. Release
model packs contain only the backend-specific `.pte` program. Beat This! code
and published weights are distributed under the MIT license.
The license text is retained in `LICENSE-MIT.txt`.
