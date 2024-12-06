use crate::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::{Read, Write};

pub fn write_to_file<T: Serialize>(obj: &T, filename: &str) -> Result<usize> {
  let encoded = bincode::serialize(obj)?;
  let mut file = OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(filename)?;
  file.write_all(&encoded)?;
  Ok(encoded.len())
}

pub fn read_from_file<T: DeserializeOwned>(filename: &str) -> Result<T> {
  let mut file = OpenOptions::new().read(true).open(filename)?;
  let mut buffer = Vec::with_capacity(8 * 1024);
  file.read_to_end(&mut buffer)?;
  let decoded = bincode::deserialize(&buffer)?;
  Ok(decoded)
}
