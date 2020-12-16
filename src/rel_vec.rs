use crate::error::Error;
use rand::rngs::ThreadRng;
use rand::Rng;
use std::io;
use std::io::{BufRead, BufReader, BufWriter};
use std::path::Path;
use std::{cmp::Ordering, io::Read};
use std::{fs::File, io::Write};

static FILE_PREFIX: [u8; 2] = [173, 42];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelEntry {
    pub name: String,
    pub wins: u32,
    pub votes: u32,
}

impl RelEntry {
    pub fn new(name: String, wins: u32, votes: u32) -> RelEntry {
        RelEntry { name, wins, votes }
    }

    pub fn percentage(&self) -> f64 {
        f64::from(self.wins) * 100.0 / f64::from(self.votes)
    }
}

impl PartialEq for RelEntry {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl ToString for RelEntry {
    fn to_string(&self) -> String {
        format!(
            "{} - {}/{} - {}%",
            self.name,
            self.wins,
            self.votes,
            self.percentage()
        )
    }
}

impl From<String> for RelEntry {
    fn from(s: String) -> Self {
        Self::new(s, 0, 0)
    }
}

#[derive(Clone, Debug)]
pub struct RelVec {
    pub inner: Vec<RelEntry>,
    rng: ThreadRng,
}

impl RelVec {
    pub fn new() -> Self {
        Self {
            inner: Vec::new(),
            rng: rand::thread_rng(),
        }
    }

    pub fn create(names: Vec<String>) -> Self {
        Self {
            inner: names.into_iter().map(|s| RelEntry::new(s, 0, 0)).collect(),
            rng: rand::thread_rng(),
        }
    }

    pub fn from<P: AsRef<Path>>(file: P) -> Result<Self, io::Error> {
        let f = File::open(file)?;
        let reader = BufReader::new(f);

        Ok(Self {
            inner: reader
                .lines()
                .map(|r| r.map(|s| RelEntry::new(s, 0, 0)))
                .collect::<Result<Vec<RelEntry>, io::Error>>()?,
            rng: rand::thread_rng(),
        })
    }

    pub fn load<P: AsRef<Path>>(file: P) -> Result<Self, Error> {
        let f = File::open(file)?;
        let mut reader = BufReader::new(f);

        {
            let mut buf = [0u8; 2];
            reader.read(&mut buf)?;

            if buf != FILE_PREFIX {
                return Err(Error::InvalidFileError);
            }
        }

        Ok(Self {
            inner: bincode::deserialize_from(reader)?,
            rng: rand::thread_rng(),
        })
    }

    pub fn save<P: AsRef<Path>>(&self, file: P) -> Result<(), Error> {
        let f = File::create(file)?;
        let mut writer = BufWriter::new(f);

        writer.write(&FILE_PREFIX)?;

        bincode::serialize_into(writer, &self.inner)?;
        Ok(())
    }

    pub fn add(&mut self, name: String) {
        self.inner.push(name.into());
    }

    pub fn sort_percentage(&mut self) {
        self.inner
            .sort_by(|a: &RelEntry, b: &RelEntry| -> Ordering {
                let ap = a.percentage();
                let bp = b.percentage();

                match bp.partial_cmp(&ap) {
                    Some(ordering) => ordering,
                    None => {
                        if ap.is_nan() {
                            if bp.is_nan() {
                                match a.name.partial_cmp(&b.name) {
                                    Some(ordering) => ordering,
                                    None => Ordering::Equal,
                                }
                            } else {
                                Ordering::Greater
                            }
                        } else if bp.is_nan() {
                            Ordering::Less
                        } else {
                            match a.name.partial_cmp(&b.name) {
                                Some(ordering) => ordering,
                                None => Ordering::Equal,
                            }
                        }
                    }
                }
            })
    }

    pub fn min_votes(&mut self) -> Vec<&mut RelEntry> {
        let mut min = u32::max_value();
        let mut v = Vec::new();

        for item in &mut self.inner {
            match item.votes.cmp(&min) {
                Ordering::Less => {
                    min = item.votes;
                    v = Vec::new();
                    v.push(item);
                }
                Ordering::Equal => {
                    v.push(item);
                }
                Ordering::Greater => {}
            }
        }

        v
    }

    pub fn random_pair(&mut self) -> Option<(&mut RelEntry, &mut RelEntry)> {
        if self.inner.len() < 2 {
            return None;
        }

        let i1 = self.rng.gen_range(0, self.inner.len());
        let mut i2 = self.rng.gen_range(0, self.inner.len() - 1);
        if i2 >= i1 {
            i2 += 1;
        }

        if i1 < i2 {
            let (a, b) = self.inner.split_at_mut(i2);

            Some((&mut a[i1], &mut b[0]))
        } else {
            let (a, b) = self.inner.split_at_mut(i1);

            Some((&mut b[0], &mut a[i2]))
        }
    }
}

impl PartialEq for RelVec {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::{BufWriter, Write},
    };

    use super::{RelEntry, RelVec};

    #[test]
    fn rel_entry_new() {
        assert_eq!(
            RelEntry {
                name: "abc".to_owned(),
                wins: 125132,
                votes: 12551
            },
            RelEntry::new("abc".to_owned(), 125132, 12551)
        )
    }

    #[test]
    fn rel_entry_partial_eq() {
        let a = RelEntry {
            name: "abc".to_owned(),
            wins: 125132,
            votes: 1263,
        };
        let b = RelEntry {
            name: "abc".to_owned(),
            wins: 1251,
            votes: 1361621,
        };

        assert_eq!(a, b);
    }

    #[test]
    fn rel_entry_to_string() {
        let a = RelEntry {
            name: "abc".to_owned(),
            wins: 12,
            votes: 36,
        };

        assert_eq!(a.to_string(), "abc - 12/36 - 33.333333333333336%");
    }

    #[test]
    fn rel_entry_from_string() {
        let a = RelEntry {
            name: "abc".to_owned(),
            wins: 0,
            votes: 0,
        };
        let b = "abc".to_owned().into();

        assert_eq!(a, b);
    }

    #[test]
    fn rel_vec_new() {
        let a = RelVec {
            inner: Vec::new(),
            rng: rand::thread_rng(),
        };
        let b = RelVec::new();

        assert_eq!(a, b);
    }

    #[test]
    fn rel_vec_create() {
        let a = RelVec {
            inner: Vec::from([
                RelEntry::new("abc".to_string(), 1251, 16162),
                RelEntry::new("adsga".to_string(), 1251, 1236),
            ]),
            rng: rand::thread_rng(),
        };
        let b = RelVec::create(Vec::from(["abc".to_string(), "adsga".to_string()]));

        assert_eq!(a, b);
    }

    #[test]
    fn rel_vec_from() {
        let file = File::create("_rel_vec_from.txt").unwrap();
        let mut writer = BufWriter::new(file);

        writer.write_all(b"a\nb\nc\n").unwrap();

        drop(writer);

        let a = RelVec::create(["a".to_string(), "b".to_string(), "c".to_string()].to_vec());
        let b = RelVec::from("_rel_vec_from.txt").unwrap();

        fs::remove_file("_rel_vec_from.txt").unwrap();

        assert_eq!(a, b);
    }

    #[test]
    fn rel_vec_load() {
        let file = File::create("_rel_vec_load.txt").unwrap();
        let mut writer = BufWriter::new(file);

        writer
            .write_all(&[
                0xad, 0x2a, 0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x3, 0x0, 0x0, 0x0, 0x0, 0x0,
                0x0, 0x0, 0x61, 0x62, 0x63, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            ])
            .unwrap();

        drop(writer);

        let a = RelVec::create(["abc".to_string()].to_vec());
        let b = RelVec::load("_rel_vec_load.txt").unwrap();

        fs::remove_file("_rel_vec_load.txt").unwrap();

        assert_eq!(a, b);
    }

    #[test]
    fn rel_vec_save() {
        let rv = RelVec::create(["abc".to_string()].to_vec());
        rv.save("_rel_vec_save.txt").unwrap();

        let a: [u8; 29] = [
            0xad, 0x2a, 0x1, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x3, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
            0x0, 0x61, 0x62, 0x63, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0,
        ];
        let b = fs::read("_rel_vec_save.txt").unwrap();

        fs::remove_file("_rel_vec_save.txt").unwrap();

        assert_eq!(&a, b.as_slice());
    }

    #[test]
    fn rel_vec_min_votes() {
        let mut rv = RelVec {
            inner: [
                RelEntry {
                    name: "abc".to_string(),
                    wins: 12,
                    votes: 123,
                },
                RelEntry {
                    name: "bcd".to_string(),
                    wins: 125,
                    votes: 123,
                },
                RelEntry {
                    name: "cde".to_string(),
                    wins: 12,
                    votes: 12632,
                },
            ]
            .to_vec(),
            rng: rand::thread_rng(),
        };

        assert_eq!(
            rv.min_votes(),
            Vec::from([
                &mut RelEntry {
                    name: "abc".to_string(),
                    wins: 12,
                    votes: 123,
                },
                &mut RelEntry {
                    name: "bcd".to_string(),
                    wins: 125,
                    votes: 123
                }
            ])
        );
    }

    #[test]
    fn rel_vec_random_pair() {
        let mut rv = RelVec {
            inner: [
                RelEntry {
                    name: "abc".to_string(),
                    wins: 0,
                    votes: 0,
                },
                RelEntry {
                    name: "def".to_string(),
                    wins: 0,
                    votes: 0,
                },
            ]
            .to_vec(),
            rng: rand::thread_rng(),
        };

        let (a, b) = rv.random_pair().unwrap();
        let (c, d) = (
            &mut RelEntry {
                name: "abc".to_string(),
                wins: 0,
                votes: 0,
            },
            &mut RelEntry {
                name: "def".to_string(),
                wins: 0,
                votes: 0,
            },
        );

        assert!((&a, &b) == (&c, &d) || (&a, &b) == (&d, &c));
    }
}
