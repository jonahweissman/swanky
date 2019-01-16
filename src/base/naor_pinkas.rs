use super::{ObliviousTransfer, Stream};
use curve25519_dalek::constants;
use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::scalar::Scalar;
use rand::rngs::ThreadRng;
use std::cmp::max;
use std::io::{Error, Read, Write};

pub struct NaorPinkasOT<T: Read + Write> {
    stream: Stream<T>,
    rng: ThreadRng,
}

const P: RistrettoPoint = constants::RISTRETTO_BASEPOINT_POINT;

impl<T: Read + Write> NaorPinkasOT<T> {
    pub fn new(stream: T) -> Self {
        let stream = Stream::new(stream);
        let rng = rand::thread_rng();
        Self { stream, rng }
    }
}

impl<T: Read + Write> ObliviousTransfer for NaorPinkasOT<T> {
    fn send(&mut self, values: (&[u8], &[u8])) -> Result<(), Error> {
        let length = max(values.0.len(), values.1.len());
        let c = RistrettoPoint::random(&mut self.rng);
        self.stream.write_pt(&c)?;
        let pk0 = self.stream.read_pt()?;
        let pk1 = c - pk0;
        let r0 = Scalar::random(&mut self.rng);
        let r1 = Scalar::random(&mut self.rng);
        let e00 = P * r0;
        let e10 = P * r1;
        let h = super::hash_pt(&(pk0 * r0), length);
        let e01 = super::xor(&h, values.0);
        let h = super::hash_pt(&(pk1 * r1), length);
        let e11 = super::xor(&h, values.1);
        self.stream.write_pt(&e00)?;
        self.stream.write_bytes(&e01)?;
        self.stream.write_pt(&e10)?;
        self.stream.write_bytes(&e11)?;
        Ok(())
    }

    fn receive(&mut self, input: bool, length: usize) -> Result<Vec<u8>, Error> {
        let c = self.stream.read_pt()?;
        let k = Scalar::random(&mut self.rng);
        let pkσ = P * k;
        let pkσ_ = c - pkσ;
        match input {
            false => self.stream.write_pt(&pkσ)?,
            true => self.stream.write_pt(&pkσ_)?,
        };
        let e00 = self.stream.read_pt()?;
        let e01 = self.stream.read_bytes(length)?;
        let e10 = self.stream.read_pt()?;
        let e11 = self.stream.read_bytes(length)?;
        let (eσ0, eσ1) = match input {
            false => (e00, e01),
            true => (e10, e11),
        };
        let h = super::hash_pt(&(eσ0 * k), length);
        let result = super::xor(&h, &eσ1);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    extern crate test;
    use super::*;
    use std::os::unix::net::UnixStream;
    use test::Bencher;

    const N: usize = 32;

    #[test]
    fn test() {
        let m0 = rand::random::<[u8; N]>();
        let m1 = rand::random::<[u8; N]>();
        let b = rand::random::<bool>();
        let (sender, receiver) = match UnixStream::pair() {
            Ok((s1, s2)) => (s1, s2),
            Err(e) => {
                eprintln!("Couldn't create pair of sockets: {:?}", e);
                return;
            }
        };
        std::thread::spawn(move || {
            let mut ot = NaorPinkasOT::new(sender);
            ot.send((&m0, &m1)).unwrap();
        });
        let mut ot = NaorPinkasOT::new(receiver);
        let result = ot.receive(b, N).unwrap();
        assert_eq!(result, if b { m1 } else { m0 });
    }

    #[bench]
    fn bench(b: &mut Bencher) {
        b.iter(|| test())
    }
}
