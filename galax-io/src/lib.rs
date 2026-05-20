use galax_core::BodiesSoA;
use std::io::{Read, Write};
use thiserror::Error;

const MAGIC: [u8; 4] = [b'G', b'A', b'L', b'X'];
const VERSION: u32 = 1;

#[derive(Error, Debug)]
pub enum SnapshotError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid magic bytes (not a galax snapshot)")]
    BadMagic,
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u32),
    #[error("Unexpected end of file")]
    UnexpectedEof,
}

/// Write BodiesSoA to a binary stream.
pub fn write_snapshot(writer: &mut impl Write, bodies: &BodiesSoA) -> Result<(), SnapshotError> {
    let n: u64 = bodies.len() as u64;
    let ts: f64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    writer.write_all(&MAGIC)?;
    writer.write_all(&VERSION.to_le_bytes())?;
    writer.write_all(&n.to_le_bytes())?;
    writer.write_all(&ts.to_le_bytes())?;

    write_f64_array(writer, &bodies.x)?;
    write_f64_array(writer, &bodies.y)?;
    write_f64_array(writer, &bodies.vx)?;
    write_f64_array(writer, &bodies.vy)?;
    write_f64_array(writer, &bodies.mass)?;
    write_f64_array(writer, &bodies.ax)?;
    write_f64_array(writer, &bodies.ay)?;

    Ok(())
}

fn write_f64_array(writer: &mut impl Write, data: &[f64]) -> Result<(), SnapshotError> {
    let bytes = unsafe {
        std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 8)
    };
    writer.write_all(bytes)?;
    Ok(())
}

/// Read a BodiesSoA from a binary stream.
pub fn read_snapshot(reader: &mut impl Read) -> Result<BodiesSoA, SnapshotError> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(SnapshotError::BadMagic);
    }

    let mut version_bytes = [0u8; 4];
    reader.read_exact(&mut version_bytes)?;
    let version = u32::from_le_bytes(version_bytes);
    if version != VERSION {
        return Err(SnapshotError::UnsupportedVersion(version));
    }

    let mut n_bytes = [0u8; 8];
    reader.read_exact(&mut n_bytes)?;
    let n = u64::from_le_bytes(n_bytes) as usize;

    let mut _timestamp_bytes = [0u8; 8];
    reader.read_exact(&mut _timestamp_bytes)?;

    let mut bodies = BodiesSoA::new(n);
    read_f64_array(reader, &mut bodies.x)?;
    read_f64_array(reader, &mut bodies.y)?;
    read_f64_array(reader, &mut bodies.vx)?;
    read_f64_array(reader, &mut bodies.vy)?;
    read_f64_array(reader, &mut bodies.mass)?;
    read_f64_array(reader, &mut bodies.ax)?;
    read_f64_array(reader, &mut bodies.ay)?;

    Ok(bodies)
}

fn read_f64_array(reader: &mut impl Read, data: &mut [f64]) -> Result<(), SnapshotError> {
    let buf = unsafe {
        std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut u8, data.len() * 8)
    };
    reader.read_exact(buf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_round_trip() {
        let n = 100;
        let mut bodies = BodiesSoA::new(n);
        for i in 0..n {
            bodies.x[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.y[i] = fastrand::f64() * 100.0 - 50.0;
            bodies.vx[i] = fastrand::f64() * 10.0 - 5.0;
            bodies.vy[i] = fastrand::f64() * 10.0 - 5.0;
            bodies.mass[i] = fastrand::f64() * 10.0 + 0.1;
            bodies.ax[i] = fastrand::f64() * 2.0 - 1.0;
            bodies.ay[i] = fastrand::f64() * 2.0 - 1.0;
        }

        let mut buf = Vec::new();
        write_snapshot(&mut buf, &bodies).unwrap();
        let restored = read_snapshot(&mut buf.as_slice()).unwrap();

        assert_eq!(restored.len(), n);
        for i in 0..n {
            assert!((restored.x[i] - bodies.x[i]).abs() < 1e-14);
            assert!((restored.y[i] - bodies.y[i]).abs() < 1e-14);
            assert!((restored.vx[i] - bodies.vx[i]).abs() < 1e-14);
            assert!((restored.vy[i] - bodies.vy[i]).abs() < 1e-14);
            assert!((restored.mass[i] - bodies.mass[i]).abs() < 1e-14);
            assert!((restored.ax[i] - bodies.ax[i]).abs() < 1e-14);
            assert!((restored.ay[i] - bodies.ay[i]).abs() < 1e-14);
        }
    }

    #[test]
    fn test_snapshot_bad_magic() {
        let bad_data = b"BADXsomejunkdata";
        let result = read_snapshot(&mut &bad_data[..]);
        assert!(matches!(result, Err(SnapshotError::BadMagic)));
    }
}
