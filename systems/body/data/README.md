# Data in raea-sys-grab

`soma_hand_prior.bin`: the left hand's articulation prior from NVIDIA's SOMA-X (github.com/NVlabs/SOMA-X,
`assets/SOMAHand.npz`, Apache-2.0, Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES), cut down to the fifteen
finger joints raea's rig has. SOMA-X's file holds a principal-component model of hand poses (32 components over 24
finger joints, each joint a rotation vector from the bind pose in the wrist's axes). Kept here, as little-endian
f32 in this order:

| numbers | what |
|---|---|
| 45 | the mean pose: thumb 1 to 3, then index, middle, ring and pinky 2 to 4 (SOMA's 1 is the metacarpal), x y z each |
| 32 | each component's standard deviation (the square root of its eigenvalue) |
| 32 x 45 | the components, on the same fifteen joints |
| 32 x 45 | the matrix that reads a pose's coefficients back (least squares on the kept joints) |
| 45 | each kept joint's bone direction at SOMA's bind pose, in the wrist's axes |

The metacarpals and the fingertip end joints are left out: raea's hand has neither. Made once from the file above
with a script that reads the .npz with Python's standard library; `src/prior.rs` reads it.
