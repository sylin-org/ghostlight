//! `form_input` -- set a form value by element ref. **Mutate** tier.
//!
//! Content-script implementation with **shadow-DOM traversal** (`findInputInside`) and the native
//! prototype-setter trick so framework-controlled inputs register the change. Implemented Phase 3.
