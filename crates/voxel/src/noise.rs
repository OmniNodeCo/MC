//! Tiny deterministic hash / noise library.
//!
//! Everything here is integer based (plus f32 interpolation) so that the same
//! seed produces byte-identical terrain natively and in the Wasm build.

/// Integer avalanche hash (based on the "lowbias32" finaliser).
#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
pub fn hash2(seed: u32, x: i32, z: i32) -> u32 {
    let a = hash_u32(x as u32 ^ 0x9e37_79b1);
    let b = hash_u32(z as u32 ^ 0x85eb_ca6b);
    hash_u32(seed ^ a.wrapping_mul(0x27d4_eb2f) ^ b.wrapping_mul(0x1656_67b1))
}

#[inline]
pub fn hash3(seed: u32, x: i32, y: i32, z: i32) -> u32 {
    let a = hash_u32(x as u32 ^ 0x9e37_79b1);
    let b = hash_u32(y as u32 ^ 0xc2b2_ae35);
    let c = hash_u32(z as u32 ^ 0x85eb_ca6b);
    hash_u32(
        seed ^ a.wrapping_mul(0x27d4_eb2f)
            ^ b.wrapping_mul(0x1656_67b1)
            ^ c.wrapping_mul(0x2545_f491),
    )
}

/// Map a hash to `0.0..1.0`.
#[inline]
pub fn rand01(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Small fast PRNG (xorshift64*).
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        let s = hash_u32(seed) as u64 | 1;
        Self {
            state: s ^ 0x9e37_79b9_7f4a_7c15,
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        (x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 32) as u32
    }

    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u32() % ((hi - lo + 1) as u32)) as i32
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.next_f32() < p
    }

    pub fn pick(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u32() as usize) % n
        }
    }
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[inline]
fn grad2(h: u32, x: f32, z: f32) -> f32 {
    match h & 7 {
        0 => x + z,
        1 => -x + z,
        2 => x - z,
        3 => -x - z,
        4 => x,
        5 => -x,
        6 => z,
        _ => -z,
    }
}

#[inline]
fn grad3(h: u32, x: f32, y: f32, z: f32) -> f32 {
    match h % 12 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

/// Classic Perlin noise, output roughly in `-1.0..1.0`.
pub fn perlin2(seed: u32, x: f32, z: f32) -> f32 {
    let xi = x.floor();
    let zi = z.floor();
    let fx = x - xi;
    let fz = z - zi;
    let (xi, zi) = (xi as i32, zi as i32);

    let g00 = grad2(hash2(seed, xi, zi), fx, fz);
    let g10 = grad2(hash2(seed, xi + 1, zi), fx - 1.0, fz);
    let g01 = grad2(hash2(seed, xi, zi + 1), fx, fz - 1.0);
    let g11 = grad2(hash2(seed, xi + 1, zi + 1), fx - 1.0, fz - 1.0);

    let u = fade(fx);
    let v = fade(fz);
    lerp(lerp(g00, g10, u), lerp(g01, g11, u), v) * 1.41
}

/// 3D Perlin noise, output roughly in `-1.0..1.0`.
pub fn perlin3(seed: u32, x: f32, y: f32, z: f32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let zi = z.floor();
    let fx = x - xi;
    let fy = y - yi;
    let fz = z - zi;
    let (xi, yi, zi) = (xi as i32, yi as i32, zi as i32);

    let c000 = grad3(hash3(seed, xi, yi, zi), fx, fy, fz);
    let c100 = grad3(hash3(seed, xi + 1, yi, zi), fx - 1.0, fy, fz);
    let c010 = grad3(hash3(seed, xi, yi + 1, zi), fx, fy - 1.0, fz);
    let c110 = grad3(hash3(seed, xi + 1, yi + 1, zi), fx - 1.0, fy - 1.0, fz);
    let c001 = grad3(hash3(seed, xi, yi, zi + 1), fx, fy, fz - 1.0);
    let c101 = grad3(hash3(seed, xi + 1, yi, zi + 1), fx - 1.0, fy, fz - 1.0);
    let c011 = grad3(hash3(seed, xi, yi + 1, zi + 1), fx, fy - 1.0, fz - 1.0);
    let c111 = grad3(
        hash3(seed, xi + 1, yi + 1, zi + 1),
        fx - 1.0,
        fy - 1.0,
        fz - 1.0,
    );

    let u = fade(fx);
    let v = fade(fy);
    let w = fade(fz);

    let x00 = lerp(c000, c100, u);
    let x10 = lerp(c010, c110, u);
    let x01 = lerp(c001, c101, u);
    let x11 = lerp(c011, c111, u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w) * 1.2
}

/// Fractal sum of Perlin octaves, roughly `-1.0..1.0`.
pub fn fbm2(seed: u32, x: f32, z: f32, octaves: u32) -> f32 {
    let mut total = 0.0;
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for i in 0..octaves {
        total += perlin2(seed.wrapping_add(i * 1013), x * freq, z * freq) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    if norm > 0.0 {
        total / norm
    } else {
        0.0
    }
}

/// Smooth value noise in `0.0..1.0`.
pub fn value2(seed: u32, x: f32, z: f32) -> f32 {
    let xi = x.floor();
    let zi = z.floor();
    let fx = fade(x - xi);
    let fz = fade(z - zi);
    let (xi, zi) = (xi as i32, zi as i32);
    let v00 = rand01(hash2(seed, xi, zi));
    let v10 = rand01(hash2(seed, xi + 1, zi));
    let v01 = rand01(hash2(seed, xi, zi + 1));
    let v11 = rand01(hash2(seed, xi + 1, zi + 1));
    lerp(lerp(v00, v10, fx), lerp(v01, v11, fx), fz)
}

/// `t` smoothly remapped from `edge0..edge1` to `0..1`.
#[inline]
pub fn smoothstep(edge0: f32, edge1: f32, t: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        return if t < edge0 { 0.0 } else { 1.0 };
    }
    let x = ((t - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_deterministic() {
        assert_eq!(hash3(7, 1, 2, 3), hash3(7, 1, 2, 3));
        assert_ne!(hash3(7, 1, 2, 3), hash3(7, 1, 2, 4));
    }

    #[test]
    fn noise_stays_in_range() {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for i in 0..2000 {
            let x = i as f32 * 0.37;
            let z = i as f32 * 0.11;
            let n = fbm2(1, x, z, 4);
            min = min.min(n);
            max = max.max(n);
        }
        assert!(min > -1.05 && max < 1.05, "{min} {max}");
    }

    #[test]
    fn rng_is_reproducible() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..16 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }
}
