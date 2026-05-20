# mui

**mui** crate provides a simple drop guard for [MaybeUninit](https://doc.rust-lang.org/stable/std/mem/union.MaybeUninit.html) (*hereinafter abbreviated as* MUI) which avoids a memory leak and a sensitive data leakage by ensuring to clear its underlying data on drop.
