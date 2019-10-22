use crate::frames::Frames;
use std::collections::HashMap;
use std::fmt::{Display, Error as FmtError, Formatter};

pub struct Report {
    data: HashMap<Frames, i32>,
}

impl From<&HashMap<Frames, i32>> for Report {
    fn from(data: &HashMap<Frames, i32>) -> Self {
        Self { data: data.clone() }
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        for (key, val) in self.data.iter() {
            write!(f, "{} {}", key, val)?;
        }

        Ok(())
    }
}
