/*
 * Panopticon - A libre disassembler
 * Copyright (C) 2014-2015 Kai Michaelis
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

use std::collections::HashMap;
use std::path::Path;
use std::fs::File;
use std::io::{Result,Read};
use std::ops::Range;

pub type Cell = Option<u8>;

#[derive(Debug,RustcDecodable,RustcEncodable)]
pub enum OpaqueLayer {
    Undefined(u64),
    Defined(Box<Vec<u8>>),
}

#[derive(Clone,Debug)]
pub enum LayerIter<'a> {
    Undefined(u64),
    Defined(Option<&'a [u8]>),
    Sparse{ map: &'a HashMap<u64,Cell>, mapped: Box<LayerIter<'a>>, pos: u64 },
    Concat{ car: Box<LayerIter<'a>>, cdr: Box<LayerIter<'a>> },
}

impl<'a> Iterator for LayerIter<'a> {
    type Item = Cell;

    fn next(&mut self) -> Option<Cell> {
        match self {
            &mut LayerIter::Undefined(0) => None,
            &mut LayerIter::Undefined(ref mut r) => { *r -= 1; Some(None) },
            &mut LayerIter::Defined(None) => None,
            &mut LayerIter::Defined(ref mut maybe_buf) => {
                let ret = maybe_buf.unwrap().first();
                let l = maybe_buf.unwrap().len();

                if l > 1 {
                    *maybe_buf = Some(&maybe_buf.unwrap()[1..l]);
                } else {
                    *maybe_buf = None;
                }
                ret.map(|&x| Some(x))
            },
            &mut LayerIter::Sparse{ map: ref m, mapped: ref mut i, pos: ref mut p } => {
                if let Some(covered) = i.next() {
                    *p += 1;
                    Some(*m.get(&(*p - 1)).unwrap_or(&covered))
                } else {
                    None
                }
            },
            &mut LayerIter::Concat{ car: ref mut a, cdr: ref mut b } => {
                if let Some(aa) = a.next() {
                    Some(aa)
                } else {
                    b.next()
                }
            },
        }
    }
}

impl<'a> Read for LayerIter<'a> {
    fn read(&mut self,buf: &mut [u8]) -> Result<usize> {
        for idx in 0..buf.len() {
            if let Some(Some(b)) = Self::next(self) {
                buf[idx] = b;
            } else {
                return Ok(idx);
            }
        }

        Ok(0)
    }
}

impl<'a> LayerIter<'a> {
    pub fn cut(&self, r: &Range<u64>) -> LayerIter<'a> {
        if r.start >= r.end {
            return LayerIter::Defined(None);
        }

        let real_end = if r.end > self.len() {
            self.len()
        } else {
            r.end
        };

        match self {
            &LayerIter::Undefined(_) => LayerIter::Undefined(real_end - r.start),
            &LayerIter::Defined(None) => LayerIter::Defined(None),
            &LayerIter::Defined(Some(ref buf)) => LayerIter::Defined(Some(&buf[r.start as usize..real_end as usize])),
            &LayerIter::Sparse{ map: ref m, mapped: ref i, pos: ref p, .. } => LayerIter::Sparse{
                map: m,
                mapped: Box::new(i.cut(r)),
                pos: p + r.start
            },
            &LayerIter::Concat{ car: ref a, cdr: ref b } => {
                if r.start < a.len() && real_end <= a.len() {
                    a.cut(r)
                } else if r.start >= a.len() && real_end > a.len() {
                    b.cut(&((r.start - a.len())..(real_end - a.len())))
                } else {
                    LayerIter::Concat{
                        car: Box::new(a.cut(&(r.start..a.len()))),
                        cdr: Box::new(b.cut(&(0..(real_end - a.len())))),
                    }
                }
            },
        }
    }

    pub fn seek(&self, p: u64) -> LayerIter<'a> {
        if p > 0 {
            self.cut(&(p..self.len()))
        } else {
            self.clone()
        }
    }

    pub fn append(&self, l: LayerIter<'a>) -> LayerIter<'a> {
        LayerIter::Concat{ car: Box::new(self.clone()), cdr: Box::new(l) }
    }

    pub fn len(&self) -> u64 {
        match self {
            &LayerIter::Undefined(r) => r,
            &LayerIter::Defined(None) => 0,
            &LayerIter::Defined(Some(ref r)) => r.len() as u64,
            &LayerIter::Sparse{ mapped: ref m, .. } => m.len(),
            &LayerIter::Concat{ car: ref a, cdr: ref b } => a.len() + b.len(),
        }
    }
}

#[derive(Debug,RustcDecodable,RustcEncodable)]
pub enum Layer {
    Opaque(OpaqueLayer),
    Sparse(HashMap<u64,Cell>)
}

impl OpaqueLayer {
    pub fn iter(&self) -> LayerIter {
        match self {
            &OpaqueLayer::Undefined(ref len) => LayerIter::Undefined(*len),
            &OpaqueLayer::Defined(ref v) => LayerIter::Defined(Some(v)),
        }
    }

    pub fn len(&self) -> u64 {
        match self {
            &OpaqueLayer::Undefined(ref len) => *len,
            &OpaqueLayer::Defined(ref v) => v.len() as u64,
        }
    }

    pub fn open(p: &Path) -> Option<OpaqueLayer> {
        let fd = File::open(p);

        if fd.is_ok() {
            let mut buf = Vec::<u8>::new();
            let len = fd.unwrap().read_to_end(&mut buf);

            if len.is_ok() {
                Some(Self::wrap(buf))
            } else {
                error!("can't read file '{:?}': {:?}",p,len);
                None
            }
        } else {
            error!("can't open file '{:?}",p);
            None
        }
    }

    pub fn wrap(d: Vec<u8>) -> OpaqueLayer {
        OpaqueLayer::Defined(Box::new(d))
    }

    pub fn undefined(l: u64) -> OpaqueLayer {
        OpaqueLayer::Undefined(l)
    }
}

impl Layer {
    pub fn filter<'a>(&'a self,i: LayerIter<'a>) -> LayerIter<'a> {
        match self {
            &Layer::Opaque(ref o) => o.iter(),
            &Layer::Sparse(ref m) => LayerIter::Sparse{ map: m, mapped: Box::new(i), pos: 0 },
        }
    }

    pub fn wrap(d: Vec<u8>) -> Layer {
        Layer::Opaque(OpaqueLayer::wrap(d))
    }

    pub fn undefined(l: u64) -> Layer {
        Layer::Opaque(OpaqueLayer::undefined(l))
    }

    pub fn open(p: &Path) -> Option<Layer> {
        OpaqueLayer::open(p).map(|x| Layer::Opaque(x))
    }

    pub fn writable() -> Layer {
        Layer::Sparse(HashMap::new())
    }

    pub fn write(&mut self, p: u64, c: Cell) -> bool {
        match self {
            &mut Layer::Sparse(ref mut m) => { m.insert(p,c); true },
            _ => false
        }
    }

    pub fn is_undefined(&self) -> bool {
        if let &Layer::Opaque(OpaqueLayer::Undefined(_)) = self {
            true
        } else {
            false
        }
    }

    pub fn is_writeable(&self) -> bool {
        if let &Layer::Sparse(_) = self {
            true
        } else {
            false
        }
    }

    pub fn as_opaque<'a>(&'a self) -> Option<&'a OpaqueLayer> {
        match self {
            &Layer::Opaque(ref o) => Some(o),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct() {
        let l1 = OpaqueLayer::undefined(6);
        let l2 = OpaqueLayer::wrap(vec!(1,2,3));

        assert_eq!(l1.len(),6);
        assert_eq!(l2.len(),3);
    }

    #[test]
    fn append() {
        let l1 = OpaqueLayer::undefined(6);
        let l2 = OpaqueLayer::wrap(vec!(1,2,3));
        let l3 = OpaqueLayer::wrap(vec!(1,2,3));
        let l4 = OpaqueLayer::wrap(vec!(13,23,33,6,7));

        let s1 = l1.iter().append(l2.iter()).append(l3.iter()).append(l4.iter());

        assert_eq!(s1.collect::<Vec<Cell>>(), vec!(None,None,None,None,None,None,Some(1),Some(2),Some(3),Some(1),Some(2),Some(3),Some(13),Some(23),Some(33),Some(6),Some(7)));
    }

    #[test]
    fn slab() {
        let l1 = OpaqueLayer::wrap(vec!(1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16));
        let mut s1 = l1.iter();

        assert_eq!(s1.len(), 16);
        assert_eq!(s1.next().unwrap(), Some(1));
        assert_eq!(s1.next().unwrap(), Some(2));
        assert_eq!(s1.len(), 14);
    }

    #[test]
    fn mutable() {
        let l1 = OpaqueLayer::wrap(vec!(1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16));
        let mut l2 = Layer::writable();
        let e = vec!(Some(1),Some(2),Some(3),Some(4),Some(5),Some(1),Some(1),Some(8),Some(9),Some(10),Some(11),Some(12),Some(13),Some(1),Some(15),Some(16));

        l2.write(5,Some(1));
        l2.write(6,Some(1));
        l2.write(13,Some(1));

        let s = l2.filter(l1.iter());
        assert_eq!(s.clone().len(), 16);
        assert_eq!(s.collect::<Vec<Cell>>(),e);
    }

    #[test]
    fn random_access_iter()
    {
        let l1 = OpaqueLayer::undefined(0xffffffff);
        let sl = l1.iter();

        // unused -> auto i = sl.begin();
        // unused -> slab::iterator j = i + 0xc0000000;

        let mut k = 100;
        while k > 0 {
            sl.append(sl.clone());
            k -= 1;
        }
    }
}
