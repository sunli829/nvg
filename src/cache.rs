use crate::context::{Command, Path, Vertex};
use crate::{Bounds, LineCap, LineJoin, Point, Solidity};
use clamped::Clamp;
use rawpointer::ptrdistance;
use std::f32::consts::PI;

bitflags! {
    #[derive(Default)]
    struct PointFlags: u32 {
        const PT_CORNER = 0x1;
        const PT_LEFT = 0x2;
        const PT_BEVEL = 0x4;
        const PR_INNERBEVEL	= 0x8;
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub(crate) struct VPoint {
    xy: Point,
    d: Point,
    len: f32,
    dm: Point,
    flags: PointFlags,
}

#[derive(Default)]
pub(crate) struct PathCache {
    pub(crate) points: Vec<VPoint>,
    pub(crate) paths: Vec<Path>,
    pub(crate) vertexes: Vec<Vertex>,
    pub(crate) bounds: Bounds,
}

impl PathCache {
    pub fn clear(&mut self) {
        self.points.clear();
        self.paths.clear();
    }

    fn add_path(&mut self) -> &mut Path {
        self.paths.push(Path {
            first: self.points.len(),
            count: 0,
            closed: false,
            num_bevel: 0,
            solidity: Solidity::Solid,
            fill: std::ptr::null_mut(),
            num_fill: 0,
            stroke: std::ptr::null_mut(),
            num_stroke: 0,
            convex: false,
        });
        self.paths.last_mut().unwrap()
    }

    fn add_point(&mut self, pt: Point, flags: PointFlags, dist_tol: f32) {
        if let Some(path) = self.paths.last_mut() {
            if let Some(last_pt) = self.points.last_mut() {
                if path.count > 0 {
                    if last_pt.xy.equals(pt, dist_tol) {
                        last_pt.flags |= flags;
                        return;
                    }
                }
            }

            self.points.push(VPoint {
                xy: pt,
                d: Default::default(),
                len: 0.0,
                dm: Default::default(),
                flags,
            });
            path.count += 1;
        }
    }

    fn close_path(&mut self) {
        if let Some(path) = self.paths.last_mut() {
            path.closed = true;
        }
    }

    fn path_solidity(&mut self, solidity: Solidity) {
        if let Some(path) = self.paths.last_mut() {
            path.solidity = solidity;
        }
    }

    unsafe fn alloc_temp_vertexes(&mut self, count: usize) -> *mut Vertex {
        self.vertexes.resize(count, Default::default());
        if self.vertexes.is_empty() {
            return std::ptr::null_mut();
        }
        &mut self.vertexes[0] as *mut Vertex
    }

    fn tesselate_bezier(
        &mut self,
        pt1: Point,
        pt2: Point,
        pt3: Point,
        pt4: Point,
        level: usize,
        flags: PointFlags,
        tess_tol: f32,
    ) {
        if level > 10 {
            return;
        }

        let Point { x: x1, y: y1 } = pt1;
        let Point { x: x2, y: y2 } = pt2;
        let Point { x: x3, y: y3 } = pt3;
        let Point { x: x4, y: y4 } = pt4;

        let x12 = (x1 + x2) * 0.5;
        let y12 = (y1 + y2) * 0.5;
        let x23 = (x2 + x3) * 0.5;
        let y23 = (y2 + y3) * 0.5;
        let x34 = (x3 + x4) * 0.5;
        let y34 = (y3 + y4) * 0.5;
        let x123 = (x12 + x23) * 0.5;
        let y123 = (y12 + y23) * 0.5;

        let dx = x4 - x1;
        let dy = y4 - y1;
        let d2 = ((x2 - x4) * dy - (y2 - y4) * dx).abs();
        let d3 = ((x3 - x4) * dy - (y3 - y4) * dx).abs();

        if (d2 + d3) * (d2 + d3) < tess_tol * (dx * dx + dy * dy) {
            self.add_point(Point::new(x4, y4), flags, tess_tol);
            return;
        }

        let x234 = (x23 + x34) * 0.5;
        let y234 = (y23 + y34) * 0.5;
        let x1234 = (x123 + x234) * 0.5;
        let y1234 = (y123 + y234) * 0.5;

        self.tesselate_bezier(
            Point::new(x1, y1),
            Point::new(x12, y12),
            Point::new(x123, y123),
            Point::new(x1234, y1234),
            level + 1,
            PointFlags::empty(),
            tess_tol,
        );
        self.tesselate_bezier(
            Point::new(x1234, y1234),
            Point::new(x234, y234),
            Point::new(x34, y34),
            Point::new(x4, y4),
            level + 1,
            flags,
            tess_tol,
        );
    }

    pub(crate) fn flatten_paths(&mut self, commands: &[Command], dist_tol: f32, tess_tol: f32) {
        for cmd in commands {
            match cmd {
                Command::MoveTo(pt) => {
                    self.add_path();
                    self.add_point(*pt, PointFlags::PT_CORNER, dist_tol);
                }
                Command::LineTo(pt) => {
                    self.add_point(*pt, PointFlags::PT_CORNER, dist_tol);
                }
                Command::BezierTo(cp1, cp2, pt) => {
                    if let Some(last) = self.points.last().map(|pt| *pt) {
                        self.tesselate_bezier(
                            last.xy,
                            *cp1,
                            *cp2,
                            *pt,
                            0,
                            PointFlags::PT_CORNER,
                            tess_tol,
                        );
                    }
                }
                Command::Close => self.close_path(),
                Command::Solidity(solidity) => self.path_solidity(*solidity),
            }
        }

        self.bounds.min = Point::new(std::f32::MAX, std::f32::MAX);
        self.bounds.max = Point::new(std::f32::MIN, std::f32::MIN);

        unsafe {
            for j in 0..self.paths.len() {
                let path = &mut self.paths[j];
                let pts = &mut self.points[path.first] as *mut VPoint;
                let mut p0 = pts.offset(path.count as isize - 1);
                let mut p1 = pts;

                if (*p0).xy.equals((*p1).xy, dist_tol) {
                    path.count -= 1;
                    p0 = pts.offset(path.count as isize - 1);
                    path.closed = true;
                }

                if path.count > 2 {
                    let area = poly_area(std::slice::from_raw_parts(pts, path.count));
                    if path.solidity == Solidity::Solid && area < 0.0 {
                        poly_reverse(std::slice::from_raw_parts_mut(pts, path.count));
                    }
                    if path.solidity == Solidity::Hole && area > 0.0 {
                        poly_reverse(std::slice::from_raw_parts_mut(pts, path.count));
                    }
                }

                for _ in 0..path.count {
                    (*p0).d.x = (*p1).xy.x - (*p0).xy.x;
                    (*p0).d.y = (*p1).xy.y - (*p0).xy.y;
                    (*p0).len = (*p0).d.normalize();

                    self.bounds.min.x = self.bounds.min.x.min((*p0).xy.x);
                    self.bounds.min.y = self.bounds.min.y.min((*p0).xy.y);
                    self.bounds.max.x = self.bounds.max.x.max((*p0).xy.x);
                    self.bounds.max.y = self.bounds.max.y.max((*p0).xy.y);

                    p0 = p1;
                    p1 = p1.add(1);
                }
            }
        }
    }

    fn calculate_joins(&mut self, w: f32, line_join: LineJoin, miter_limit: f32) {
        let mut iw = 0.0;
        if w > 0.0 {
            iw = 1.0 / w;
        }

        unsafe {
            for i in 0..self.paths.len() {
                let path = &mut self.paths[i];
                let pts = &mut self.points[path.first] as *mut VPoint;
                let mut p0 = pts.offset(path.count as isize - 1);
                let mut p1 = pts;
                let mut nleft = 0;

                path.num_bevel = 0;

                for _ in 0..path.count {
                    let dlx0 = (*p0).d.y;
                    let dly0 = -(*p0).d.x;
                    let dlx1 = (*p1).d.y;
                    let dly1 = -(*p1).d.x;

                    (*p1).dm.x = (dlx0 + dlx1) * 0.5;
                    (*p1).dm.y = (dly0 + dly1) * 0.5;
                    let dmr2 = (*p1).dm.x * (*p1).dm.x + (*p1).dm.y * (*p1).dm.y;

                    if dmr2 > 0.000001 {
                        let mut scale = 1.0 / dmr2;
                        if scale > 600.0 {
                            scale = 600.0;
                        }
                        (*p1).dm.x *= scale;
                        (*p1).dm.y *= scale;
                    }

                    (*p1).flags &= PointFlags::PT_CORNER;

                    let cross = (*p1).d.x * (*p0).d.y - (*p0).d.x * (*p1).d.y;
                    if cross > 0.0 {
                        nleft += 1;
                        (*p1).flags |= PointFlags::PT_LEFT;
                    }

                    let limit = (((*p0).len.min((*p1).len) as f32) * iw).max(1.01);
                    if (dmr2 * limit * limit) < 1.0 {
                        (*p1).flags |= PointFlags::PR_INNERBEVEL;
                    }

                    if (*p1).flags.contains(PointFlags::PT_CORNER) {
                        if (dmr2 * miter_limit * miter_limit) < 1.0
                            || line_join == LineJoin::Bevel
                            || line_join == LineJoin::Round
                        {
                            (*p1).flags |= PointFlags::PT_BEVEL;
                        }
                    }

                    if (*p1).flags.contains(PointFlags::PT_BEVEL)
                        || (*p1).flags.contains(PointFlags::PR_INNERBEVEL)
                    {
                        path.num_bevel += 1;
                    }

                    p0 = p1;
                    p1 = p1.add(1);
                }

                path.convex = nleft == path.count;
            }
        }
    }

    pub(crate) fn expand_stroke(
        &mut self,
        mut w: f32,
        fringe: f32,
        line_cap: LineCap,
        line_join: LineJoin,
        miter_limit: f32,
        tess_tol: f32,
    ) {
        let aa = fringe;
        let mut u0 = 0.0;
        let mut u1 = 1.0;
        let ncap = curve_divs(w, PI, tess_tol);

        w += aa * 0.5;

        if aa == 0.0 {
            u0 = 0.5;
            u1 = 0.5;
        }

        self.calculate_joins(w, line_join, miter_limit);

        let mut cverts = 0;
        for path in &self.paths {
            let loop_ = path.closed;
            if line_join == LineJoin::Round {
                cverts += (path.count + path.num_bevel * (ncap + 2) + 1) * 2;
            } else {
                cverts += (path.count + path.num_bevel * 5 + 1) * 2;
                if !loop_ {
                    if line_cap == LineCap::Round {
                        cverts += (ncap * 2 + 2) * 2;
                    } else {
                        cverts += (3 + 3) * 2;
                    }
                }
            }
        }

        unsafe {
            let mut vertexes = self.alloc_temp_vertexes(cverts);
            if vertexes.is_null() {
                return;
            }

            for i in 0..self.paths.len() {
                let path = &mut self.paths[i];
                let pts = &mut self.points[path.first] as *mut VPoint;

                path.fill = std::ptr::null_mut();
                path.num_fill = 0;

                let loop_ = path.closed;
                let mut dst = vertexes;
                path.stroke = dst;

                let (mut p0, mut p1, s, e) = if loop_ {
                    (pts.offset(path.count as isize - 1), pts, 0, path.count)
                } else {
                    (pts, pts.add(1), 1, path.count - 1)
                };

                if !loop_ {
                    let mut d = Point::new((*p1).xy.x - (*p0).xy.x, (*p1).xy.y - (*p0).xy.y);
                    d.normalize();
                    match line_cap {
                        LineCap::Butt => {
                            dst = butt_cap_start(
                                dst,
                                p0.as_mut().unwrap(),
                                d.x,
                                d.y,
                                w,
                                -aa * 0.5,
                                aa,
                                u0,
                                u1,
                            )
                        }
                        LineCap::Square => {
                            dst = butt_cap_start(
                                dst,
                                p0.as_mut().unwrap(),
                                d.x,
                                d.y,
                                w,
                                w - aa,
                                aa,
                                u0,
                                u1,
                            )
                        }
                        LineCap::Round => {
                            dst = round_cap_start(
                                dst,
                                p0.as_mut().unwrap(),
                                d.x,
                                d.y,
                                w,
                                ncap,
                                aa,
                                u0,
                                u1,
                            )
                        }
                    }
                }

                for _ in s..e {
                    if (*p1).flags.contains(PointFlags::PT_BEVEL)
                        || (*p1).flags.contains(PointFlags::PR_INNERBEVEL)
                    {
                        if line_join == LineJoin::Round {
                            dst = round_join(
                                dst,
                                p0.as_mut().unwrap(),
                                p1.as_mut().unwrap(),
                                w,
                                w,
                                u0,
                                u1,
                                ncap,
                                aa,
                            );
                        } else {
                            dst = bevel_join(
                                dst,
                                p0.as_mut().unwrap(),
                                p1.as_mut().unwrap(),
                                w,
                                w,
                                u0,
                                u1,
                                aa,
                            );
                        }
                    } else {
                        *dst = Vertex::new(
                            (*p1).xy.x + ((*p1).dm.x * w),
                            (*p1).xy.y + ((*p1).dm.y * w),
                            u0,
                            1.0,
                        );
                        dst = dst.add(1);

                        *dst = Vertex::new(
                            (*p1).xy.x - ((*p1).dm.x * w),
                            (*p1).xy.y - ((*p1).dm.y * w),
                            u1,
                            1.0,
                        );
                        dst = dst.add(1);
                    }
                    p0 = p1;
                    p1 = p1.add(1);
                }

                if loop_ {
                    let v0 = vertexes;
                    let v1 = vertexes.add(1);

                    *dst = Vertex::new((*v0).x, (*v0).y, u0, 1.0);
                    dst = dst.add(1);

                    *dst = Vertex::new((*v1).x, (*v1).y, u1, 1.0);
                    dst = dst.add(1);
                } else {
                    let mut d = Point::new((*p1).xy.x - (*p0).xy.x, (*p1).xy.y - (*p0).xy.y);
                    d.normalize();
                    match line_cap {
                        LineCap::Butt => {
                            dst = butt_cap_end(
                                dst,
                                p1.as_mut().unwrap(),
                                d.x,
                                d.y,
                                w,
                                -aa * 0.5,
                                aa,
                                u0,
                                u1,
                            );
                        }
                        LineCap::Round => {
                            dst = butt_cap_end(
                                dst,
                                p1.as_mut().unwrap(),
                                d.x,
                                d.y,
                                w,
                                w - aa,
                                aa,
                                u0,
                                u1,
                            );
                        }
                        LineCap::Square => {
                            dst = round_cap_end(
                                dst,
                                p1.as_mut().unwrap(),
                                d.x,
                                d.y,
                                w,
                                ncap,
                                aa,
                                u0,
                                u1,
                            );
                        }
                    }
                }

                path.num_stroke = ptrdistance(vertexes, dst);
                vertexes = dst;
            }
        }
    }

    pub(crate) fn expand_fill(
        &mut self,
        w: f32,
        line_join: LineJoin,
        miter_limit: f32,
        fringe_width: f32,
    ) {
        let aa = fringe_width;
        let fringe = w > 0.0;

        self.calculate_joins(w, line_join, miter_limit);

        let mut cverts = 0;
        for path in &self.paths {
            cverts += path.count + path.num_bevel + 1;
            if fringe {
                cverts += (path.count + path.num_bevel * 5 + 1) * 2;
            }
        }

        unsafe {
            let mut vertexes = self.alloc_temp_vertexes(cverts);
            if vertexes.is_null() {
                return;
            }

            let convex = self.paths.len() == 1 && self.paths[0].convex;

            for i in 0..self.paths.len() {
                let path = &mut self.paths[i];
                let pts = &mut self.points[path.first] as *mut VPoint;
                let woff = 0.5 * aa;
                let mut dst = vertexes;

                path.fill = dst;

                if fringe {
                    let mut p0 = pts.offset(path.count as isize - 1);
                    let mut p1 = pts;
                    for _ in 0..path.count {
                        if (*p1).flags.contains(PointFlags::PT_BEVEL) {
                            let dlx0 = (*p0).d.y;
                            let dly0 = -(*p0).d.x;
                            let dlx1 = (*p1).d.y;
                            let dly1 = -(*p1).d.x;
                            if (*p1).flags.contains(PointFlags::PT_LEFT) {
                                let lx = (*p1).xy.x + (*p1).dm.x * woff;
                                let ly = (*p1).xy.y + (*p1).dm.y * woff;
                                *dst = Vertex::new(lx, ly, 0.5, 1.0);
                                dst = dst.add(1);
                            } else {
                                let lx0 = (*p1).xy.x + dlx0 * woff;
                                let ly0 = (*p1).xy.y + dly0 * woff;
                                let lx1 = (*p1).xy.x + dlx1 * woff;
                                let ly1 = (*p1).xy.y + dly1 * woff;

                                *dst = Vertex::new(lx0, ly0, 0.5, 1.0);
                                dst = dst.add(1);

                                *dst = Vertex::new(lx1, ly1, 0.5, 1.0);
                                dst = dst.add(1);
                            }
                        } else {
                            *dst = Vertex::new(
                                (*p1).xy.x + ((*p1).dm.x * woff),
                                (*p1).xy.y + ((*p1).dm.y * woff),
                                0.5,
                                1.0,
                            );
                            dst = dst.add(1);
                        }

                        p0 = p1;
                        p1 = p1.add(1);
                    }
                } else {
                    for j in 0..path.count {
                        let pt = pts.add(j);
                        *dst = Vertex::new((*pt).xy.x, (*pt).xy.y, 0.5, 1.0);
                        dst = dst.add(1);
                    }
                }

                path.num_fill = ptrdistance(vertexes, dst);
                vertexes = dst;

                if fringe {
                    let mut lw = w + woff;
                    let rw = w - woff;
                    let mut lu = 0.0;
                    let ru = 1.0;
                    let mut dst = vertexes;
                    path.stroke = dst;

                    if convex {
                        lw = woff;
                        lu = 0.5;
                    }

                    let mut p0 = pts.offset(path.count as isize - 1);
                    let mut p1 = pts;

                    for _ in 0..path.count {
                        if (*p1).flags.contains(PointFlags::PT_BEVEL)
                            || (*p1).flags.contains(PointFlags::PR_INNERBEVEL)
                        {
                            dst = bevel_join(
                                dst,
                                p0.as_mut().unwrap(),
                                p1.as_mut().unwrap(),
                                lw,
                                rw,
                                lu,
                                ru,
                                fringe_width,
                            );
                        } else {
                            *dst = Vertex::new(
                                (*p1).xy.x + ((*p1).dm.x * lw),
                                (*p1).xy.y + ((*p1).dm.y * lw),
                                lu,
                                1.0,
                            );
                            dst = dst.add(1);

                            *dst = Vertex::new(
                                (*p1).xy.x - ((*p1).dm.x * rw),
                                (*p1).xy.y - ((*p1).dm.y * rw),
                                ru,
                                1.0,
                            );
                            dst = dst.add(1);
                        }
                        p0 = p1;
                        p1 = p1.add(1);
                    }

                    let v0 = vertexes;
                    let v1 = vertexes.add(1);

                    *dst = Vertex::new((*v0).x, (*v0).y, lu, 1.0);
                    dst = dst.add(1);

                    *dst = Vertex::new((*v1).x, (*v1).y, ru, 1.0);
                    dst = dst.add(1);

                    path.num_stroke = ptrdistance(vertexes, dst);
                    vertexes = dst;
                } else {
                    path.stroke = std::ptr::null_mut();
                    path.num_stroke = 0;
                }
            }
        }
    }
}

fn triangle_area(a: &VPoint, b: &VPoint, c: &VPoint) -> f32 {
    let a = &a.xy;
    let b = &b.xy;
    let c = &c.xy;
    let abx = b.x - a.x;
    let aby = b.y - a.y;
    let acx = c.x - a.x;
    let acy = c.y - a.y;
    acx * aby - abx * acy
}

fn poly_area(pts: &[VPoint]) -> f32 {
    let mut area = 0.0;
    for i in 2..pts.len() {
        let a = &pts[0];
        let b = &pts[i - 1];
        let c = &pts[i];
        area += triangle_area(a, b, c);
    }
    area * 0.5
}

fn poly_reverse(pts: &mut [VPoint]) {
    let mut i = 0;
    let mut j = pts.len() as i32 - 1;
    while i < j {
        pts.swap(i as usize, j as usize);
        i += 1;
        j -= 1;
    }
}

fn curve_divs(r: f32, arc: f32, tess_tol: f32) -> usize {
    let da = (r / (r + tess_tol)).acos() * 2.0;
    ((arc / da).ceil() as i32).max(2) as usize
}

fn choose_bevel(bevel: bool, p0: &mut VPoint, p1: &mut VPoint, w: f32) -> (f32, f32, f32, f32) {
    if bevel {
        let x0 = p1.xy.x + p0.d.y * w;
        let y0 = p1.xy.y - p0.d.x * w;
        let x1 = p1.xy.x + p1.d.y * w;
        let y1 = p1.xy.y - p1.d.x * w;
        (x0, y0, x1, y1)
    } else {
        let x0 = p1.xy.x + p1.dm.x * w;
        let y0 = p1.xy.y + p1.dm.y * w;
        let x1 = p1.xy.x + p1.dm.x * w;
        let y1 = p1.xy.y + p1.dm.y * w;
        (x0, y0, x1, y1)
    }
}

unsafe fn round_join(
    mut dst: *mut Vertex,
    p0: &mut VPoint,
    p1: &mut VPoint,
    lw: f32,
    rw: f32,
    lu: f32,
    ru: f32,
    ncap: usize,
    _fringe: f32,
) -> *mut Vertex {
    let dlx0 = p0.d.y;
    let dly0 = -p0.d.x;
    let dlx1 = p1.d.y;
    let dly1 = -p1.d.x;

    if p1.flags.contains(PointFlags::PT_LEFT) {
        let (lx0, ly0, lx1, ly1) =
            choose_bevel(p1.flags.contains(PointFlags::PR_INNERBEVEL), p0, p1, lw);
        let a0 = -dly0.atan2(-dlx0);
        let mut a1 = -dly1.atan2(-dlx1);
        if a1 > a0 {
            a1 -= PI * 2.0;
        }

        *dst = Vertex::new(lx0, ly0, lu, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(p1.xy.x - dlx0 * rw, p1.xy.y - dly0 * rw, ru, 1.0);
        dst = dst.add(1);

        let n = ((((a0 - a1) / PI) * (ncap as f32)).ceil() as i32).clamped(2, ncap as i32);
        for i in 0..n {
            let u = (i as f32) / ((n - 1) as f32);
            let a = a0 + u * (a1 - a0);
            let rx = p1.xy.x + a.cos() * rw;
            let ry = p1.xy.y + a.sin() * rw;

            *dst = Vertex::new(p1.xy.x, p1.xy.y, 0.5, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(rx, ry, ru, 1.0);
            dst = dst.add(1);
        }

        *dst = Vertex::new(lx1, ly1, lu, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(p1.xy.x - dlx1 * rw, p1.xy.y - dly1 * rw, ru, 1.0);
        dst = dst.add(1);
    } else {
        let (rx0, ry0, rx1, ry1) =
            choose_bevel(p1.flags.contains(PointFlags::PR_INNERBEVEL), p0, p1, -rw);
        let a0 = dly0.atan2(dlx0);
        let mut a1 = dly1.atan2(dlx1);
        if a1 < a0 {
            a1 += PI * 2.0;
        }

        *dst = Vertex::new(p1.xy.x + dlx0 * rw, p1.xy.y + dly0 * rw, lu, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(rx0, ry0, ru, 1.0);
        dst = dst.add(1);

        let n = ((((a0 - a1) / PI) * (ncap as f32)).ceil() as i32).clamped(2, ncap as i32);
        for i in 0..n {
            let u = (i as f32) / ((n - 1) as f32);
            let a = a0 + u * (a1 - a0);
            let lx = p1.xy.x + a.cos() * lw;
            let ly = p1.xy.y + a.cos() * lw;

            *dst = Vertex::new(lx, ly, lu, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x, p1.xy.y, 0.5, 1.0);
            dst = dst.add(1);
        }

        *dst = Vertex::new(p1.xy.x + dlx1 * rw, p1.xy.y + dly1 * rw, lu, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(rx1, ry1, ru, 1.0);
        dst = dst.add(1);
    }

    dst
}

unsafe fn bevel_join(
    mut dst: *mut Vertex,
    p0: &mut VPoint,
    p1: &mut VPoint,
    lw: f32,
    rw: f32,
    lu: f32,
    ru: f32,
    _fringe: f32,
) -> *mut Vertex {
    let dlx0 = p0.d.y;
    let dly0 = -p0.d.x;
    let dlx1 = p1.d.y;
    let dly1 = -p1.d.x;

    if p1.flags.contains(PointFlags::PT_LEFT) {
        let (lx0, ly0, lx1, ly1) =
            choose_bevel(p1.flags.contains(PointFlags::PR_INNERBEVEL), p0, p1, lw);

        *dst = Vertex::new(lx0, ly0, lu, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(p1.xy.x - dlx0 * rw, p1.xy.y - dly0 * rw, ru, 1.0);
        dst = dst.add(1);

        if p1.flags.contains(PointFlags::PT_BEVEL) {
            *dst = Vertex::new(lx0, ly0, lu, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x - dlx0 * rw, p1.xy.y - dly0 * rw, ru, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(lx1, ly1, lu, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x - dlx1 * rw, p1.xy.y - dly1 * rw, ru, 1.0);
            dst = dst.add(1);
        } else {
            let rx0 = p1.xy.x - p1.dm.x * rw;
            let ry0 = p1.xy.y - p1.dm.y * rw;

            *dst = Vertex::new(p1.xy.x, p1.xy.y, 0.5, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x - dlx0 * rw, p1.xy.y - dly0 * rw, ru, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(rx0, ry0, ru, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(rx0, ry0, ru, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x, p1.xy.y, 0.5, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x - dlx1 * rw, p1.xy.y - dly1 * rw, ru, 1.0);
            dst = dst.add(1);
        }

        *dst = Vertex::new(lx1, ly1, lu, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(p1.xy.x - dlx1 * rw, p1.xy.y - dly1 * rw, ru, 1.0);
        dst = dst.add(1);
    } else {
        let (rx0, ry0, rx1, ry1) =
            choose_bevel(p1.flags.contains(PointFlags::PR_INNERBEVEL), p0, p1, -rw);

        *dst = Vertex::new(p1.xy.x + dlx0 * lw, p1.xy.y + dly0 * lw, lu, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(rx0, ry0, ru, 1.0);
        dst = dst.add(1);

        if p1.flags.contains(PointFlags::PT_BEVEL) {
            *dst = Vertex::new(p1.xy.x + dlx0 * lw, p1.xy.y + dly0 * lw, lu, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(rx0, ry0, ru, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x + dlx1 * lw, p1.xy.y + dly1 * lw, lu, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(rx1, ry1, ru, 1.0);
            dst = dst.add(1);
        } else {
            let lx0 = p1.xy.x + p1.dm.x * lw;
            let ly0 = p1.xy.y + p1.dm.y * lw;

            *dst = Vertex::new(p1.xy.x + dlx0 * lw, p1.xy.y + dly0 * lw, lu, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x, p1.xy.y, 0.5, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(lx0, ly0, lu, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(lx0, ly0, lu, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x + dlx1 * lw, p1.xy.y + dly1 * lw, lu, 1.0);
            dst = dst.add(1);

            *dst = Vertex::new(p1.xy.x, p1.xy.y, 0.5, 1.0);
            dst = dst.add(1);
        }

        *dst = Vertex::new(p1.xy.x + dlx1 * lw, p1.xy.y + dly1 * lw, lu, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(rx1, ry1, ru, 1.0);
        dst = dst.add(1);
    }

    dst
}

unsafe fn butt_cap_start(
    mut dst: *mut Vertex,
    p: &mut VPoint,
    dx: f32,
    dy: f32,
    w: f32,
    d: f32,
    aa: f32,
    u0: f32,
    u1: f32,
) -> *mut Vertex {
    let px = p.xy.x - dx * d;
    let py = p.xy.y - dy * d;
    let dlx = dy;
    let dly = -dx;

    *dst = Vertex::new(px + dlx * w - dx * aa, py + dly * w - dy * aa, u0, 0.0);
    dst = dst.add(1);

    *dst = Vertex::new(px - dlx * w - dx * aa, py - dly * w - dy * aa, u1, 0.0);
    dst = dst.add(1);

    *dst = Vertex::new(px + dlx * w, py + dly * w, u0, 1.0);
    dst = dst.add(1);

    *dst = Vertex::new(px - dlx * w, py - dly * w, u1, 1.0);
    dst = dst.add(1);

    dst
}

unsafe fn butt_cap_end(
    mut dst: *mut Vertex,
    p: &mut VPoint,
    dx: f32,
    dy: f32,
    w: f32,
    d: f32,
    aa: f32,
    u0: f32,
    u1: f32,
) -> *mut Vertex {
    let px = p.xy.x - dx * d;
    let py = p.xy.y - dy * d;
    let dlx = dy;
    let dly = -dx;

    *dst = Vertex::new(px + dlx * w, py + dly * w, u0, 1.0);
    dst = dst.add(1);

    *dst = Vertex::new(px - dlx * w, py - dly * w, u1, 1.0);
    dst = dst.add(1);

    *dst = Vertex::new(px + dlx * w + dx * aa, py + dly * w + dy * aa, u0, 0.0);
    dst = dst.add(1);

    *dst = Vertex::new(px - dlx * w + dx * aa, py - dly * w + dy * aa, u1, 0.0);
    dst = dst.add(1);

    dst
}

unsafe fn round_cap_start(
    mut dst: *mut Vertex,
    p: &mut VPoint,
    dx: f32,
    dy: f32,
    w: f32,
    ncap: usize,
    _aa: f32,
    u0: f32,
    u1: f32,
) -> *mut Vertex {
    let px = p.xy.x;
    let py = p.xy.y;
    let dlx = dy;
    let dly = -dx;

    for i in 0..ncap {
        let a = (i as f32) / ((ncap - 1) as f32) * PI;
        let ax = a.cos() * w;
        let ay = a.sin() * w;

        *dst = Vertex::new(px - dlx * ax - dx * ay, py - dly * ax - dy * ay, u0, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(px, py, 0.5, 1.0);
        dst = dst.add(1);
    }

    *dst = Vertex::new(px + dlx * w, py + dly * w, u0, 1.0);
    dst = dst.add(1);

    *dst = Vertex::new(px - dlx * w, py - dly * w, u1, 1.0);
    dst = dst.add(1);

    dst
}

unsafe fn round_cap_end(
    mut dst: *mut Vertex,
    p: &mut VPoint,
    dx: f32,
    dy: f32,
    w: f32,
    ncap: usize,
    _aa: f32,
    u0: f32,
    u1: f32,
) -> *mut Vertex {
    let px = p.xy.x;
    let py = p.xy.y;
    let dlx = dy;
    let dly = -dx;

    *dst = Vertex::new(px + dlx * w, py + dly * w, u0, 1.0);
    dst = dst.add(1);

    *dst = Vertex::new(px - dlx * w, py - dly * w, u1, 1.0);
    dst = dst.add(1);

    for i in 0..ncap {
        let a = (i as f32) / ((ncap - 1) as f32) * PI;
        let ax = a.cos() * w;
        let ay = a.sin() * w;

        *dst = Vertex::new(px, py, 0.5, 1.0);
        dst = dst.add(1);

        *dst = Vertex::new(px - dlx * ax + dx * ay, py - dly * ax + dy * ay, u0, 1.0);
        dst = dst.add(1);
    }

    dst
}
