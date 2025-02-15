use std::io::{Cursor, Read, Seek, SeekFrom};

use byteorder::{BigEndian, ReadBytesExt};

use crate::db::GameVersion;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ResrcId {
    pub resrc_type: [u8; 3],
    pub method: ResrcMethod,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ResrcRevision {
    pub head: u32,
    pub branch_id: u16,
    pub branch_revision: u16,
}

impl ResrcRevision {
    pub fn get_version(&self) -> u16 {
        (self.head & 0xFFFF) as u16
    }
    pub fn get_subversion(&self) -> u16 {
        ((self.head >> 16) & 0xFFFF) as u16
    }
    pub fn is_lbp1(&self) -> bool {
        self.head <= 0x272
    }
    pub fn is_lbp3(&self) -> bool {
        self.head >> 0x10 != 0
    }
    pub fn get_gameversion(&self) -> GameVersion {
        if self.is_lbp1() {
            GameVersion::Lbp1
        } else if self.is_lbp3() {
            GameVersion::Lbp3
        } else {
            GameVersion::Lbp2
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ResrcMethod {
    Null,
    Binary {
        is_encrypted: bool,
        revision: ResrcRevision,
        dependencies: Vec<ResrcDependency>,
    },
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ResrcDependency {
    pub desc: ResrcDescriptor,
    resrc_type: u32,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ResrcDescriptor {
    Sha1([u8; 20]),
    Guid(u32),
}

impl ResrcDependency {
    pub fn parse_table(res: &mut Cursor<&[u8]>) -> Vec<Self> {
        let table_offset = res.read_u32::<BigEndian>().unwrap();
        let orig_offset = res.position();

        res.seek(SeekFrom::Start(table_offset as u64)).unwrap();

        let mut dependencies = vec![];
        for _ in 0..res.read_u32::<BigEndian>().unwrap() {
            let dep_type = match res.read_u8().unwrap() {
                0 => { // lbp3 dynamic thermometer levels use this??? why??????
                    res.seek(SeekFrom::Current(4)).unwrap(); // resrc_type
                    continue;
                }, 
                1 => {
                    let mut sha1 = [0u8; 20];
                    res.read_exact(&mut sha1).unwrap();
                    ResrcDescriptor::Sha1(sha1)
                },
                2 => ResrcDescriptor::Guid(res.read_u32::<BigEndian>().unwrap()),
                _ => panic!("what the fuck???"),
            };

            let resrc_type = res.read_u32::<BigEndian>().unwrap();

            dependencies.push(Self {
                desc: dep_type,
                resrc_type,
            })
        }

        res.seek(SeekFrom::Start(orig_offset)).unwrap();

        dependencies
    }
}

impl ResrcId {
    pub fn new(res: &[u8]) -> Self {
        let mut res = Cursor::new(res);

        let mut resrc_type = [0u8; 3];
        res.read_exact(&mut resrc_type).unwrap();

        let method = res.read_u8().unwrap();

        let method = match method {
            b'b' | b'e' => {
                let mut rev = ResrcRevision {
                    head: res.read_u32::<BigEndian>().unwrap(),
                    branch_id: 0,
                    branch_revision: 0,
                };
                let dependencies = match rev.head >= 0x109 {
                    true => ResrcDependency::parse_table(&mut res),
                    false => vec![],
                };

                if resrc_type != *b"SMH" && rev.head >= 0x271 {
                    rev.branch_id = res.read_u16::<BigEndian>().unwrap();
                    rev.branch_revision = res.read_u16::<BigEndian>().unwrap();
                }

                ResrcMethod::Binary {
                    is_encrypted: method == b'e',
                    revision: rev,
                    dependencies,
                }
            },
            _ => { ResrcMethod::Null },
        };

        Self {
            resrc_type,
            method,
        }
    }
}