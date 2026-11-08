//! The details of a section as far as the model knows them (product §4.3),
//! and the references the harness reads the rest from (SPEC cycle 2 §5.2).
//!
//! The `Transcript` makes both when the details key is pressed, and sends
//! them to the `App` as a [`DetailsRequest`](crate::components::DetailsRequest);
//! the details overlay shows them. Neither component owns them, so they live
//! here, outside both.

mod rows;
mod source;

pub use rows::{ENVIRONMENT, MESSAGE, PROMPT, TOOL_CALL, TURN, section_details};
pub use source::section_source;
