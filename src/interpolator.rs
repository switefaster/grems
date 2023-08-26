pub struct Linear2DInterpolator<const N: usize> {
    data: Vec<(nalgebra::Vector2<f64>, [f32; N])>,
    triangles: Vec<usize>,
    bounding_box: [[f64; 2]; 2],
}

impl<const N: usize> Linear2DInterpolator<N> {
    pub fn new(data: Vec<(nalgebra::Vector2<f64>, [f32; N])>) -> Self {
        let mut max_x = f64::NEG_INFINITY;
        let mut min_x = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;

        let points = data
            .iter()
            .map(|(p, _)| {
                max_x = max_x.max(p.x);
                min_x = min_x.min(p.x);
                max_y = max_y.max(p.y);
                min_y = min_y.min(p.y);
                delaunator::Point { x: p.x, y: p.y }
            })
            .collect::<Vec<_>>();

        Self {
            data,
            triangles: delaunator::triangulate(&points).triangles,
            bounding_box: [[min_x, max_x], [min_y, max_y]],
        }
    }

    pub fn interpolate(&self, p: nalgebra::Vector2<f64>) -> Option<[f32; N]> {
        self.find_simplex(p).map(|(tri, bary)| {
            let d1 = self.data[self.triangles[tri * 3]].1;
            let d2 = self.data[self.triangles[tri * 3 + 1]].1;
            let d3 = self.data[self.triangles[tri * 3 + 2]].1;

            d1.into_iter()
                .zip(d2.into_iter())
                .zip(d3.into_iter())
                .map(|((d1, d2), d3)| {
                    d3 * bary[0] as f32 + d1 * bary[1] as f32 + d2 * bary[2] as f32
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        })
    }

    fn find_simplex(&self, p: nalgebra::Vector2<f64>) -> Option<(usize, [f64; 3])> {
        let eps = std::f64::EPSILON * 100.;

        if p.x < self.bounding_box[0][0] - eps
            || p.x > self.bounding_box[0][1] + eps
            || p.y < self.bounding_box[1][0] - eps
            || p.y > self.bounding_box[1][1] + eps
        {
            return None;
        }

        if self.triangles.len() <= 0 {
            return None;
        }

        for (tri, verts) in self.triangles.chunks(3).enumerate() {
            let p1 = self.data[verts[0]].0;
            let p2 = self.data[verts[1]].0;
            let p3 = self.data[verts[2]].0;

            let e1 = p2 - p1;
            let e2 = p3 - p1;

            let denom = nalgebra::Matrix2::from_columns(&[e1, e2]).determinant();

            let r1 = p1 - p;
            let r2 = p2 - p;
            let r3 = p3 - p;

            let b1 = nalgebra::Matrix2::from_columns(&[r1, r2]).determinant() / denom;
            let b2 = nalgebra::Matrix2::from_columns(&[r2, r3]).determinant() / denom;
            let b3 = 1. - b1 - b2;

            if b1 > 0. - eps && b2 > 0. - eps && b3 > 0. - eps {
                return Some((tri, [b1, b2, b3]));
            }
        }

        None
    }
}
