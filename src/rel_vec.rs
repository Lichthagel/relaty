use crate::error::Error;
use rand::Rng;
use rand::{prelude::SliceRandom, rngs::ThreadRng};
use std::cmp::Ordering;
use std::fs::File;
use std::{
    io,
    ops::{Index, IndexMut},
};
use std::{
    io::{BufRead, BufReader, BufWriter},
    ops::Deref,
};
use std::{ops::DerefMut, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelEntry {
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "w", default)]
    pub wins: u32,
    #[serde(rename = "v", default)]
    pub votes: u32,
    #[serde(rename = "l", default)]
    pub locked: bool,
}

impl RelEntry {
    pub fn new(name: String, wins: u32, votes: u32) -> RelEntry {
        RelEntry {
            name,
            wins,
            votes,
            locked: false,
        }
    }

    pub fn reset(&mut self) {
        self.wins = 0;
        self.votes = 0;
        self.locked = false;
    }

    pub fn percentage(&self) -> f64 {
        f64::from(self.wins) * 100.0 / f64::from(self.votes)
    }

    pub fn compare_percentage(&self, other: &RelEntry) -> Ordering {
        // TODO NaN should be handled different
        let ap = self.wins * other.votes;
        let bp = other.wins * self.votes;

        ap.cmp(&bp)
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
            "{} - {}/{} - {}%{}",
            self.name,
            self.wins,
            self.votes,
            self.percentage(),
            if self.locked { " [L]" } else { "" }
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
        let reader = BufReader::new(f);

        Ok(Self {
            inner: serde_json::from_reader(reader)?,
            rng: rand::thread_rng(),
        })
    }

    pub fn save<P: AsRef<Path>>(&self, file: P) -> Result<(), Error> {
        let f = File::create(file)?;
        let writer = BufWriter::new(f);

        serde_json::to_writer(writer, &self.inner)?;
        Ok(())
    }

    pub fn add(&mut self, name: String) {
        self.push(name.into());
    }

    pub fn remove<F: FnMut(&RelEntry) -> bool>(&mut self, mut filter: F) {
        self.retain(|i| !filter(i))
    }

    pub fn sort_percentage(&mut self) {
        self.sort_by(|a: &RelEntry, b: &RelEntry| a.compare_percentage(b).reverse())
    }

    pub fn reduced(&self) -> Vec<usize> {
        self.inner
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.locked)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn min_votes(&mut self) -> Vec<usize> {
        let mut min = u32::max_value();
        let mut v = Vec::new();

        for i in 0..self.len() {
            if self[i].locked {
                continue;
            }
            match self[i].votes.cmp(&min) {
                Ordering::Less => {
                    min = self[i].votes;
                    v = Vec::new();
                    v.push(i);
                }
                Ordering::Equal => {
                    v.push(i);
                }
                Ordering::Greater => {}
            }
        }

        v
    }

    pub fn random_pair(&mut self) -> Option<(usize, usize)> {
        let reduced: Vec<usize> = self.reduced();

        if reduced.len() < 2 {
            return None;
        }

        let i1 = self.rng.gen_range(0..reduced.len());
        let i2 = self.rng.gen_range(0..(reduced.len() - 1));
        if i2 >= i1 {
            Some((reduced[i1], reduced[i2 + 1]))
        } else {
            Some((reduced[i1], reduced[i2]))
        }
    }

    pub fn min_pair(&mut self) -> Option<(usize, usize)> {
        let reduced = self.reduced();
        let mins = self.min_votes();
        if reduced.len() < 2 {
            return None;
        }

        let i1 = mins[self.rng.gen_range(0..mins.len())];
        let i2 = reduced[self.rng.gen_range(0..(reduced.len() - 1))];
        if i2 >= i1 {
            Some((i1, i2 + 1))
        } else {
            Some((i1, i2))
        }
    }

    pub fn equal_pair(&mut self) -> Option<(usize, usize)> {
        let mut reduced = self.reduced();

        if reduced.len() < 2 {
            return None;
        }

        reduced.shuffle(&mut self.rng);

        for i1 in 0..reduced.len() {
            for i2 in i1 + 1..reduced.len() {
                if (self[reduced[i2]].percentage() - self[reduced[i1]].percentage()).abs()
                    < f64::EPSILON
                {
                    return Some((reduced[i1], reduced[i2]));
                }
            }
        }
        None
    }

    pub fn nearest_pair(&mut self) -> Option<(usize, usize)> {
        let mut reduced = self.reduced();

        if reduced.len() < 2 {
            return None;
        }

        reduced.shuffle(&mut self.rng);

        let mut min = None;

        for i1 in 0..reduced.len() {
            for i2 in i1 + 1..reduced.len() {
                let d2 = (self[reduced[i2]].percentage() - self[reduced[i1]].percentage()).abs();
                match min {
                    Some((_, _, d)) => {
                        if d2 < d {
                            min = Some((i1, i2, d2));
                        }
                    }
                    None => {
                        min = Some((i1, i2, d2));
                    }
                }
            }
        }

        min.map(|(a, b, _)| (reduced[a], reduced[b]))
    }

    pub fn min_equal_pair(&mut self) -> Option<(usize, usize)> {
        if self.rng.gen_bool(0.5) {
            match self.equal_pair() {
                Some((a, b)) => Some((a, b)),
                None => self.min_pair(),
            }
        } else {
            self.min_pair()
        }
    }
}

impl PartialEq for RelVec {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Deref for RelVec {
    type Target = Vec<RelEntry>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for RelVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl AsRef<Vec<RelEntry>> for RelVec {
    fn as_ref(&self) -> &Vec<RelEntry> {
        &self.inner
    }
}

impl AsMut<Vec<RelEntry>> for RelVec {
    fn as_mut(&mut self) -> &mut Vec<RelEntry> {
        &mut self.inner
    }
}

impl Index<usize> for RelVec {
    type Output = RelEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.inner[index]
    }
}

impl IndexMut<usize> for RelVec {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.inner[index]
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cmp::Ordering,
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
                votes: 12551,
                locked: false
            },
            RelEntry::new("abc".to_owned(), 125132, 12551)
        );
    }

    #[test]
    fn rel_entry_reset() {
        let a = RelEntry {
            name: "abc".to_owned(),
            wins: 0,
            votes: 0,
            locked: false,
        };
        let mut b = RelEntry {
            name: "abc".to_owned(),
            wins: 125132,
            votes: 12551,
            locked: true,
        };

        b.reset();

        assert_eq!(a, b);
    }

    #[test]
    fn rel_entry_percentage() {
        let mut e = RelEntry::new("abc".to_owned(), 0, 0);

        assert!(e.percentage().is_nan());

        e.votes = 1;

        assert_eq!(e.percentage(), 0.0);

        e.wins = 1;

        assert_eq!(e.percentage(), 100.0);

        e.votes = 2;

        assert_eq!(e.percentage(), 50.0);

        e.wins = 125;
        e.votes = 312;

        assert_eq!(e.percentage(), 40.06410256410256);
    }

    #[test]
    fn rel_entry_compare_percentage() {
        let mut a = RelEntry::new("abc".to_owned(), 0, 0);
        let mut b = RelEntry::new("abc".to_owned(), 0, 0);

        assert_eq!(a.compare_percentage(&b), Ordering::Equal);

        b.votes = 1;

        assert_eq!(a.compare_percentage(&b), Ordering::Equal);

        a.wins = 1;
        a.votes = 2;

        assert_eq!(a.compare_percentage(&b), Ordering::Greater);

        b.wins = 1;

        assert_eq!(a.compare_percentage(&b), Ordering::Less);

        b.votes = 2;

        assert_eq!(a.compare_percentage(&b), Ordering::Equal);
    }

    #[test]
    fn rel_entry_partial_eq() {
        let a = RelEntry {
            name: "abc".to_owned(),
            wins: 125132,
            votes: 1263,
            locked: false,
        };
        let b = RelEntry {
            name: "abc".to_owned(),
            wins: 1251,
            votes: 1361621,
            locked: false,
        };

        assert_eq!(a, b);
    }

    #[test]
    fn rel_entry_to_string() {
        let a = RelEntry {
            name: "abc".to_owned(),
            wins: 12,
            votes: 36,
            locked: false,
        };

        assert_eq!(a.to_string(), "abc - 12/36 - 33.333333333333336%");
    }

    #[test]
    fn rel_entry_from_string() {
        let a = RelEntry {
            name: "abc".to_owned(),
            wins: 0,
            votes: 0,
            locked: false,
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
            .write_all(b"[{\"n\":\"abc\",\"w\":2,\"v\":3}]")
            .unwrap();

        drop(writer);

        let a = RelVec {
            inner: [RelEntry::new("abc".to_owned(), 2, 3)].to_vec(),
            rng: rand::thread_rng(),
        };
        let b = RelVec::load("_rel_vec_load.txt").unwrap();

        fs::remove_file("_rel_vec_load.txt").unwrap();

        assert_eq!(a, b);
    }

    #[test]
    fn rel_vec_save() {
        let rv = RelVec::create(["abc".to_string()].to_vec());
        rv.save("_rel_vec_save.txt").unwrap();

        let a = b"[{\"n\":\"abc\",\"w\":0,\"v\":0,\"l\":false}]";
        let b = fs::read("_rel_vec_save.txt").unwrap();

        fs::remove_file("_rel_vec_save.txt").unwrap();

        assert_eq!(a, b.as_slice());
    }

    #[test]
    fn rel_vec_add() {
        let mut rv = RelVec {
            inner: Vec::new(),
            rng: rand::thread_rng(),
        };

        rv.add("abc".to_owned());

        assert_eq!(
            rv,
            RelVec {
                inner: [RelEntry::new("abc".to_owned(), 0, 0)].to_vec(),
                rng: rand::thread_rng()
            }
        )
    }

    #[test]
    fn rel_vec_remove() {
        let mut rv = RelVec {
            inner: [RelEntry::new("abc".to_owned(), 0, 0)].to_vec(),
            rng: rand::thread_rng(),
        };

        rv.remove(|entry| entry.name.len() == 3);

        assert_eq!(rv, RelVec::new());
    }

    #[test]
    fn rel_vec_sort_percentage() {
        let mut rv = RelVec {
            inner: [
                RelEntry::new("bec".to_owned(), 1, 1),
                RelEntry::new("ads".to_owned(), 1, 2),
                RelEntry::new("foo".to_owned(), 3, 4),
                RelEntry::new("bar".to_owned(), 1, 4),
                RelEntry::new("abc".to_owned(), 0, 0),
            ]
            .to_vec(),
            rng: rand::thread_rng(),
        };

        rv.sort_percentage();

        assert_eq!(
            rv,
            RelVec {
                inner: [
                    RelEntry::new("bec".to_owned(), 1, 1),
                    RelEntry::new("foo".to_owned(), 3, 4),
                    RelEntry::new("ads".to_owned(), 1, 2),
                    RelEntry::new("bar".to_owned(), 1, 4),
                    RelEntry::new("abc".to_owned(), 0, 0),
                ]
                .to_vec(),
                rng: rand::thread_rng(),
            }
        );
    }

    #[test]
    fn rel_vec_min_votes() {
        let mut rv = RelVec {
            inner: [
                RelEntry {
                    name: "abc".to_string(),
                    wins: 12,
                    votes: 123,
                    locked: false,
                },
                RelEntry {
                    name: "bcd".to_string(),
                    wins: 125,
                    votes: 123,
                    locked: false,
                },
                RelEntry {
                    name: "locked".to_string(),
                    wins: 0,
                    votes: 0,
                    locked: true,
                },
                RelEntry {
                    name: "cde".to_string(),
                    wins: 12,
                    votes: 12632,
                    locked: false,
                },
            ]
            .to_vec(),
            rng: rand::thread_rng(),
        };

        assert_eq!(rv.min_votes(), [0, 1].to_vec());
    }

    #[test]
    fn rel_vec_random_pair() {
        for _ in 0..5 {
            let mut rv = RelVec {
                inner: [
                    RelEntry {
                        name: "abc".to_string(),
                        wins: 0,
                        votes: 0,
                        locked: false,
                    },
                    RelEntry {
                        name: "locked".to_string(),
                        wins: 0,
                        votes: 0,
                        locked: true,
                    },
                    RelEntry {
                        name: "def".to_string(),
                        wins: 0,
                        votes: 0,
                        locked: false,
                    },
                ]
                .to_vec(),
                rng: rand::thread_rng(),
            };

            let (a, b) = rv.random_pair().unwrap();

            assert!((a, b) == (0, 2) || (a, b) == (2, 0));
        }
    }

    #[test]
    fn rel_vec_min_pair() {
        let mut rv = RelVec {
            inner: [
                RelEntry {
                    name: "abc".to_string(),
                    wins: 0,
                    votes: 2,
                    locked: false,
                },
                RelEntry {
                    name: "locked".to_string(),
                    wins: 0,
                    votes: 0,
                    locked: true,
                },
                RelEntry {
                    name: "def".to_string(),
                    wins: 0,
                    votes: 1,
                    locked: false,
                },
            ]
            .to_vec(),
            rng: rand::thread_rng(),
        };

        let (a, b) = rv.min_pair().unwrap();

        assert_eq!((a, b), (2, 0))
    }

    #[test]
    fn rel_vec_equal_pair() {
        let mut rv = RelVec {
            inner: [
                RelEntry {
                    name: "abc".to_string(),
                    wins: 1,
                    votes: 2,
                    locked: false,
                },
                RelEntry {
                    name: "locked".to_string(),
                    wins: 1,
                    votes: 1,
                    locked: false,
                },
                RelEntry {
                    name: "def".to_string(),
                    wins: 1,
                    votes: 2,
                    locked: false,
                },
            ]
            .to_vec(),
            rng: rand::thread_rng(),
        };

        let (a, b) = rv.equal_pair().unwrap();

        assert!((a, b) == (0, 2) || (a, b) == (2, 0));
    }

    #[test]
    fn rel_vec_equal_pair_fail() {
        let mut rv = RelVec {
            inner: [
                RelEntry {
                    name: "abc".to_string(),
                    wins: 1,
                    votes: 2,
                    locked: false,
                },
                RelEntry {
                    name: "locked".to_string(),
                    wins: 1,
                    votes: 1,
                    locked: false,
                },
                RelEntry {
                    name: "def".to_string(),
                    wins: 1,
                    votes: 2,
                    locked: true,
                },
            ]
            .to_vec(),
            rng: rand::thread_rng(),
        };

        assert_eq!(rv.equal_pair(), None);
    }

    #[test]
    fn rel_vec_nearest_pair() {
        let mut rv = RelVec {
            inner: [
                RelEntry {
                    name: "abc".to_string(),
                    wins: 1,
                    votes: 2,
                    locked: false,
                },
                RelEntry {
                    name: "locked".to_string(),
                    wins: 1,
                    votes: 1,
                    locked: true,
                },
                RelEntry {
                    name: "def".to_string(),
                    wins: 5,
                    votes: 8,
                    locked: false,
                },
            ]
            .to_vec(),
            rng: rand::thread_rng(),
        };

        let (a, b) = rv.nearest_pair().unwrap();

        assert!((a, b) == (0, 2) || (a, b) == (2, 0));
    }

    #[test]
    fn rel_vec_nearest_pair_two() {
        let mut rv = RelVec {
            inner: [
                RelEntry {
                    name: "abc".to_string(),
                    wins: 1,
                    votes: 2,
                    locked: false,
                },
                RelEntry {
                    name: "locked".to_string(),
                    wins: 1,
                    votes: 1,
                    locked: false,
                },
                RelEntry {
                    name: "def".to_string(),
                    wins: 5,
                    votes: 8,
                    locked: true,
                },
            ]
            .to_vec(),
            rng: rand::thread_rng(),
        };
        let (a, b) = rv.nearest_pair().unwrap();

        assert!((a, b) == (0, 1) || (a, b) == (1, 0));
    }
}
