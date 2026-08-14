//! Exact Rust port of the d3-force v3 subset the network map uses
//! (`platform.map.tsx`): velocity-Verlet simulation with the forces
//! registered in the reference's order — manyBody ("charge"), x, y,
//! link, radial, collide — plus the d3-quadtree 3.0.1 machinery
//! manyBody/collide traverse. Ported statement-for-statement from the
//! JS sources so the settled coordinates match the browser within the
//! CLAUDE.md 1px bar (transcendental ulps are the only drift source):
//!
//! - the shared LCG random source (a=1664525, c=1013904223, m=2^32,
//!   s0=1) and `jiggle` draws stay in the same stream order (link →
//!   manyBody → collide per tick, traversal order preserved);
//! - `velocityDecay(0.4)` stores `1 - 0.4 = 0.6` as the per-tick
//!   multiplier (the d3 setter inverts — a quirk, mirrored);
//! - quadtree `cover` doubles integer extents exactly; `visit` pushes
//!   children 3,2,1,0 (pops 0..3), `visitAfter` pushes 0..3 twice;
//! - coincident points chain head-first (`leaf.next = node`);
//!   manyBody walks the whole chain, collide sees only the head —
//!   both faithful to d3, chain order included;
//! - alpha schedule: `alphaDecay = 1 - 0.001^(1/300)`, and the
//!   reduced-motion settle is `stop(); tick(300)`.
//!
//! All arithmetic is f64, matching JS numbers; the LCG state stays
//! below 2^53 so `%` in f64 is exact.

/// One simulated node — the reference's `MapNode` physics fields.
#[derive(Clone, Debug)]
pub struct SimNode {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub fx: Option<f64>,
    pub fy: Option<f64>,
    /// Label-inclusive exclusion radius (`forceCollide` radius).
    pub collide: f64,
    /// Preferred distance from stage centre (`forceRadial` radius).
    pub radial: f64,
    pub radial_strength: f64,
}

impl SimNode {
    pub fn new(x: f64, y: f64) -> Self {
        SimNode { x, y, vx: 0.0, vy: 0.0, fx: None, fy: None, collide: 0.0, radial: 0.0, radial_strength: 0.0 }
    }
}

/// One spring — endpoints as node indices (`forceLink` resolves ids
/// before the first tick; the graph builder hands indices directly).
#[derive(Clone, Copy, Debug)]
pub struct SimLink {
    pub source: usize,
    pub target: usize,
    pub dist: f64,
    pub strength: f64,
}

/// d3-force `lcg()` — exact: products stay under 2^53.
struct Lcg {
    s: f64,
}

impl Lcg {
    #[cfg(test)]
    fn new() -> Self {
        Lcg { s: 1.0 }
    }
    fn next(&mut self) -> f64 {
        self.s = (1664525.0 * self.s + 1013904223.0) % 4294967296.0;
        self.s / 4294967296.0
    }
    fn jiggle(&mut self) -> f64 {
        (self.next() - 0.5) * 1e-6
    }
}

// ---------------------------------------------------------------------------
// d3-quadtree (the subset addAll/visit/visitAfter used by the forces)
// ---------------------------------------------------------------------------

/// Arena quad. A leaf's `data` chains coincident points head-first
/// (d3 prepends via `leaf.next = node`). `value`/`cx`/`cy` are the
/// manyBody accumulation slots; `r` is collide's per-quad max radius.
enum Quad {
    Internal([Option<usize>; 4]),
    Leaf(Vec<usize>),
}

struct QuadAux {
    value: f64,
    cx: f64,
    cy: f64,
    r: f64,
}

struct Quadtree {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    root: Option<usize>,
    quads: Vec<Quad>,
    aux: Vec<QuadAux>,
}

impl Quadtree {
    /// `quadtree(nodes, x, y).addAll` for point accessors evaluated
    /// up front (the forces pass simple projections).
    fn build(points: &[(f64, f64)]) -> Quadtree {
        let mut t = Quadtree {
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            root: None,
            quads: Vec::new(),
            aux: Vec::new(),
        };
        let mut x0 = f64::INFINITY;
        let mut y0 = f64::INFINITY;
        let mut x1 = f64::NEG_INFINITY;
        let mut y1 = f64::NEG_INFINITY;
        for &(x, y) in points {
            if x.is_nan() || y.is_nan() {
                continue;
            }
            if x < x0 {
                x0 = x;
            }
            if x > x1 {
                x1 = x;
            }
            if y < y0 {
                y0 = y;
            }
            if y > y1 {
                y1 = y;
            }
        }
        if x0 > x1 || y0 > y1 {
            return t;
        }
        t.cover(x0, y0);
        t.cover(x1, y1);
        for (i, &(x, y)) in points.iter().enumerate() {
            t.add(x, y, i, points);
        }
        t
    }

    fn new_quad(&mut self, q: Quad) -> usize {
        self.quads.push(q);
        self.aux.push(QuadAux { value: 0.0, cx: 0.0, cy: 0.0, r: 0.0 });
        self.quads.len() - 1
    }

    fn cover(&mut self, x: f64, y: f64) {
        if x.is_nan() || y.is_nan() {
            return;
        }
        let mut x0 = self.x0;
        let mut y0 = self.y0;
        let mut x1 = self.x1;
        let mut y1 = self.y1;
        if x0.is_nan() {
            x0 = x.floor();
            x1 = x0 + 1.0;
            y0 = y.floor();
            y1 = y0 + 1.0;
        } else {
            let mut z = if x1 - x0 != 0.0 { x1 - x0 } else { 1.0 };
            let mut node = self.root;
            while x0 > x || x >= x1 || y0 > y || y >= y1 {
                let i = (((y < y0) as usize) << 1) | ((x < x0) as usize);
                let parent = self.new_quad(Quad::Internal([None, None, None, None]));
                if let Quad::Internal(children) = &mut self.quads[parent] {
                    children[i] = node;
                }
                node = Some(parent);
                z *= 2.0;
                match i {
                    0 => {
                        x1 = x0 + z;
                        y1 = y0 + z;
                    }
                    1 => {
                        x0 = x1 - z;
                        y1 = y0 + z;
                    }
                    2 => {
                        x1 = x0 + z;
                        y0 = y1 - z;
                    }
                    _ => {
                        x0 = x1 - z;
                        y0 = y1 - z;
                    }
                }
            }
            if let Some(root) = self.root {
                if matches!(self.quads[root], Quad::Internal(_)) {
                    self.root = node;
                }
            }
        }
        self.x0 = x0;
        self.y0 = y0;
        self.x1 = x1;
        self.y1 = y1;
    }

    fn add(&mut self, x: f64, y: f64, d: usize, points: &[(f64, f64)]) {
        if x.is_nan() || y.is_nan() {
            return;
        }
        let mut x0 = self.x0;
        let mut y0 = self.y0;
        let mut x1 = self.x1;
        let mut y1 = self.y1;
        let Some(mut node) = self.root else {
            self.root = Some(self.new_quad(Quad::Leaf(vec![d])));
            return;
        };
        // Find the existing leaf for the new point, or add it.
        let mut parent: Option<(usize, usize)> = None;
        loop {
            match &self.quads[node] {
                Quad::Internal(children) => {
                    let xm = (x0 + x1) / 2.0;
                    let right = x >= xm;
                    if right {
                        x0 = xm;
                    } else {
                        x1 = xm;
                    }
                    let ym = (y0 + y1) / 2.0;
                    let bottom = y >= ym;
                    if bottom {
                        y0 = ym;
                    } else {
                        y1 = ym;
                    }
                    let i = ((bottom as usize) << 1) | (right as usize);
                    match children[i] {
                        Some(child) => {
                            parent = Some((node, i));
                            node = child;
                        }
                        None => {
                            let leaf = self.new_quad(Quad::Leaf(vec![d]));
                            if let Quad::Internal(children) = &mut self.quads[node] {
                                children[i] = Some(leaf);
                            }
                            return;
                        }
                    }
                }
                Quad::Leaf(chain) => {
                    let head = chain[0];
                    let (xp, yp) = points[head];
                    if x == xp && y == yp {
                        // Coincident: prepend (d3's `leaf.next = node`).
                        if let Quad::Leaf(chain) = &mut self.quads[node] {
                            chain.insert(0, d);
                        }
                        return;
                    }
                    // Split until the old and new point separate.
                    let old = node;
                    loop {
                        let internal = self.new_quad(Quad::Internal([None, None, None, None]));
                        match parent {
                            Some((p, pi)) => {
                                if let Quad::Internal(children) = &mut self.quads[p] {
                                    children[pi] = Some(internal);
                                }
                            }
                            None => self.root = Some(internal),
                        }
                        let xm = (x0 + x1) / 2.0;
                        let right = x >= xm;
                        if right {
                            x0 = xm;
                        } else {
                            x1 = xm;
                        }
                        let ym = (y0 + y1) / 2.0;
                        let bottom = y >= ym;
                        if bottom {
                            y0 = ym;
                        } else {
                            y1 = ym;
                        }
                        let i = ((bottom as usize) << 1) | (right as usize);
                        let j = (((yp >= ym) as usize) << 1) | ((xp >= xm) as usize);
                        if i == j {
                            parent = Some((internal, i));
                            continue;
                        }
                        let leaf = self.new_quad(Quad::Leaf(vec![d]));
                        if let Quad::Internal(children) = &mut self.quads[internal] {
                            children[j] = Some(old);
                            children[i] = Some(leaf);
                        }
                        return;
                    }
                }
            }
        }
    }

    /// Pre-order with pruning; children pushed 3,2,1,0 (d3's order).
    fn visit(&mut self, mut callback: impl FnMut(&mut Quadtree, usize, f64, f64, f64, f64) -> bool) {
        let mut stack: Vec<(usize, f64, f64, f64, f64)> = Vec::new();
        if let Some(root) = self.root {
            stack.push((root, self.x0, self.y0, self.x1, self.y1));
        }
        while let Some((node, x0, y0, x1, y1)) = stack.pop() {
            if !callback(self, node, x0, y0, x1, y1) {
                if let Quad::Internal(children) = &self.quads[node] {
                    let children = *children;
                    let xm = (x0 + x1) / 2.0;
                    let ym = (y0 + y1) / 2.0;
                    if let Some(c) = children[3] {
                        stack.push((c, xm, ym, x1, y1));
                    }
                    if let Some(c) = children[2] {
                        stack.push((c, x0, ym, xm, y1));
                    }
                    if let Some(c) = children[1] {
                        stack.push((c, xm, y0, x1, ym));
                    }
                    if let Some(c) = children[0] {
                        stack.push((c, x0, y0, xm, ym));
                    }
                }
            }
        }
    }

    /// Post-order (children before parents), d3's two-stack scheme.
    fn visit_after(&mut self, mut callback: impl FnMut(&mut Quadtree, usize)) {
        let mut quads: Vec<(usize, f64, f64, f64, f64)> = Vec::new();
        let mut next: Vec<usize> = Vec::new();
        if let Some(root) = self.root {
            quads.push((root, self.x0, self.y0, self.x1, self.y1));
        }
        while let Some((node, x0, y0, x1, y1)) = quads.pop() {
            if let Quad::Internal(children) = &self.quads[node] {
                let children = *children;
                let xm = (x0 + x1) / 2.0;
                let ym = (y0 + y1) / 2.0;
                if let Some(c) = children[0] {
                    quads.push((c, x0, y0, xm, ym));
                }
                if let Some(c) = children[1] {
                    quads.push((c, xm, y0, x1, ym));
                }
                if let Some(c) = children[2] {
                    quads.push((c, x0, ym, xm, y1));
                }
                if let Some(c) = children[3] {
                    quads.push((c, xm, ym, x1, y1));
                }
            }
            next.push(node);
        }
        while let Some(node) = next.pop() {
            callback(self, node);
        }
    }
}

// ---------------------------------------------------------------------------
// The map's simulation: charge, x, y, link, radial, collide — in order.
// ---------------------------------------------------------------------------

/// The reference's fixed force parameters (`useForceLayout`).
const CHARGE_STRENGTH: f64 = -140.0;
const CHARGE_DISTANCE_MAX2: f64 = 380.0 * 380.0;
const CHARGE_DISTANCE_MIN2: f64 = 1.0;
const CHARGE_THETA2: f64 = 0.81;
const XY_STRENGTH: f64 = 0.012;
/// `velocityDecay(0.4)` → internal multiplier `1 - 0.4` (d3 inverts).
const VELOCITY_DECAY: f64 = 0.6;
const ALPHA_MIN: f64 = 0.001;

pub struct Simulation {
    pub nodes: Vec<SimNode>,
    links: Vec<SimLink>,
    link_bias: Vec<f64>,
    alpha: f64,
    alpha_target: f64,
    alpha_decay: f64,
    random: Lcg,
    cx: f64,
    cy: f64,
}

impl Simulation {
    /// Mirrors the sim construction + per-snapshot re-registration in
    /// `useForceLayout` (fresh simulation, first snapshot).
    pub fn new(nodes: Vec<SimNode>, links: Vec<SimLink>, cx: f64, cy: f64) -> Simulation {
        Simulation::with_state(nodes, links, cx, cy, 1.0, 1.0)
    }

    /// Re-registration for a later snapshot of a persistent simulation:
    /// alpha and the LCG stream carry over (one d3 simulation object
    /// lives for the component's lifetime).
    pub fn with_state(
        nodes: Vec<SimNode>,
        links: Vec<SimLink>,
        cx: f64,
        cy: f64,
        alpha: f64,
        lcg_s: f64,
    ) -> Simulation {
        // forceLink.initialize: degree counts → per-link bias.
        let mut count = vec![0.0f64; nodes.len()];
        for l in &links {
            count[l.source] += 1.0;
            count[l.target] += 1.0;
        }
        let link_bias: Vec<f64> =
            links.iter().map(|l| count[l.source] / (count[l.source] + count[l.target])).collect();
        let mut sim = Simulation {
            nodes,
            links,
            link_bias,
            alpha,
            alpha_target: 0.0,
            alpha_decay: 1.0 - ALPHA_MIN.powf(1.0 / 300.0),
            random: Lcg { s: lcg_s },
            cx,
            cy,
        };
        // initializeNodes: pinned nodes snap to (fx, fy); velocities 0.
        for n in &mut sim.nodes {
            if let Some(fx) = n.fx {
                n.x = fx;
            }
            if let Some(fy) = n.fy {
                n.y = fy;
            }
        }
        sim
    }

    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// `sim.alpha(0.5)` — the reheat before an animated restart.
    pub fn set_alpha(&mut self, alpha: f64) {
        self.alpha = alpha;
    }

    pub fn lcg_state(&self) -> f64 {
        self.random.s
    }

    /// `sim.stop(); sim.tick(300)` — the reduced-motion settle.
    pub fn settle(&mut self) {
        self.tick(300);
    }

    pub fn tick(&mut self, iterations: usize) {
        for _ in 0..iterations {
            self.alpha += (self.alpha_target - self.alpha) * self.alpha_decay;
            let alpha = self.alpha;
            self.force_many_body(alpha);
            self.force_x(alpha);
            self.force_y(alpha);
            self.force_link(alpha);
            self.force_radial(alpha);
            self.force_collide();
            for n in &mut self.nodes {
                match n.fx {
                    None => {
                        n.vx *= VELOCITY_DECAY;
                        n.x += n.vx;
                    }
                    Some(fx) => {
                        n.x = fx;
                        n.vx = 0.0;
                    }
                }
                match n.fy {
                    None => {
                        n.vy *= VELOCITY_DECAY;
                        n.y += n.vy;
                    }
                    Some(fy) => {
                        n.y = fy;
                        n.vy = 0.0;
                    }
                }
            }
        }
    }

    fn force_many_body(&mut self, alpha: f64) {
        let points: Vec<(f64, f64)> = self.nodes.iter().map(|n| (n.x, n.y)).collect();
        let mut tree = Quadtree::build(&points);
        // visitAfter(accumulate): centroid + summed strength per quad.
        tree.visit_after(|t, qi| match &t.quads[qi] {
            Quad::Internal(children) => {
                let children = *children;
                let mut strength = 0.0;
                let mut weight = 0.0;
                let mut x = 0.0;
                let mut y = 0.0;
                for c in children.into_iter().flatten() {
                    let v = t.aux[c].value;
                    let cw = v.abs();
                    if cw != 0.0 {
                        strength += v;
                        weight += cw;
                        x += cw * t.aux[c].cx;
                        y += cw * t.aux[c].cy;
                    }
                }
                t.aux[qi].cx = x / weight;
                t.aux[qi].cy = y / weight;
                t.aux[qi].value = strength;
            }
            Quad::Leaf(chain) => {
                let head = chain[0];
                t.aux[qi].cx = points[head].0;
                t.aux[qi].cy = points[head].1;
                // All map nodes share the constant charge strength.
                t.aux[qi].value = CHARGE_STRENGTH * chain.len() as f64;
            }
        });
        for i in 0..self.nodes.len() {
            let nx = self.nodes[i].x;
            let ny = self.nodes[i].y;
            let mut dvx = 0.0;
            let mut dvy = 0.0;
            let random = &mut self.random;
            tree.visit(|t, qi, x1, _y1, x2, _y2| {
                let aux = &t.aux[qi];
                if aux.value == 0.0 {
                    return true;
                }
                let mut x = aux.cx - nx;
                let mut y = aux.cy - ny;
                let w = x2 - x1;
                let mut l = x * x + y * y;
                // Barnes-Hut approximation when the cell is far enough.
                if w * w / CHARGE_THETA2 < l {
                    if l < CHARGE_DISTANCE_MAX2 {
                        if x == 0.0 {
                            x = random.jiggle();
                            l += x * x;
                        }
                        if y == 0.0 {
                            y = random.jiggle();
                            l += y * y;
                        }
                        if l < CHARGE_DISTANCE_MIN2 {
                            l = (CHARGE_DISTANCE_MIN2 * l).sqrt();
                        }
                        dvx += x * aux.value * alpha / l;
                        dvy += y * aux.value * alpha / l;
                    }
                    return true;
                }
                if matches!(t.quads[qi], Quad::Internal(_)) || l >= CHARGE_DISTANCE_MAX2 {
                    return false;
                }
                let Quad::Leaf(chain) = &t.quads[qi] else { unreachable!() };
                // Limit forces for very close nodes; jiggle coincidence.
                if chain[0] != i || chain.len() > 1 {
                    if x == 0.0 {
                        x = random.jiggle();
                        l += x * x;
                    }
                    if y == 0.0 {
                        y = random.jiggle();
                        l += y * y;
                    }
                    if l < CHARGE_DISTANCE_MIN2 {
                        l = (CHARGE_DISTANCE_MIN2 * l).sqrt();
                    }
                }
                for &d in chain {
                    if d != i {
                        let w = CHARGE_STRENGTH * alpha / l;
                        dvx += x * w;
                        dvy += y * w;
                    }
                }
                false
            });
            self.nodes[i].vx += dvx;
            self.nodes[i].vy += dvy;
        }
    }

    fn force_x(&mut self, alpha: f64) {
        for n in &mut self.nodes {
            n.vx += (self.cx - n.x) * XY_STRENGTH * alpha;
        }
    }

    fn force_y(&mut self, alpha: f64) {
        for n in &mut self.nodes {
            n.vy += (self.cy - n.y) * XY_STRENGTH * alpha;
        }
    }

    fn force_link(&mut self, alpha: f64) {
        for (i, l) in self.links.iter().enumerate() {
            let (s, t) = (l.source, l.target);
            let mut x = self.nodes[t].x + self.nodes[t].vx - self.nodes[s].x - self.nodes[s].vx;
            if x == 0.0 {
                x = self.random.jiggle();
            }
            let mut y = self.nodes[t].y + self.nodes[t].vy - self.nodes[s].y - self.nodes[s].vy;
            if y == 0.0 {
                y = self.random.jiggle();
            }
            let mut len = (x * x + y * y).sqrt();
            len = (len - l.dist) / len * alpha * l.strength;
            let x = x * len;
            let y = y * len;
            let b = self.link_bias[i];
            self.nodes[t].vx -= x * b;
            self.nodes[t].vy -= y * b;
            let b = 1.0 - b;
            self.nodes[s].vx += x * b;
            self.nodes[s].vy += y * b;
        }
    }

    fn force_radial(&mut self, alpha: f64) {
        for n in &mut self.nodes {
            // strength accessor: NaN radius → 0 strength (radii here are
            // always finite; 0-strength nodes contribute nothing).
            let dx = if n.x - self.cx != 0.0 { n.x - self.cx } else { 1e-6 };
            let dy = if n.y - self.cy != 0.0 { n.y - self.cy } else { 1e-6 };
            let r = (dx * dx + dy * dy).sqrt();
            let k = (n.radial - r) * n.radial_strength * alpha / r;
            n.vx += dx * k;
            n.vy += dy * k;
        }
    }

    fn force_collide(&mut self) {
        // strength 1, iterations 3 (the reference's config).
        for _ in 0..3 {
            let points: Vec<(f64, f64)> =
                self.nodes.iter().map(|n| (n.x + n.vx, n.y + n.vy)).collect();
            let mut tree = Quadtree::build(&points);
            // visitAfter(prepare): max radius per quad.
            tree.visit_after(|t, qi| match &t.quads[qi] {
                Quad::Leaf(chain) => {
                    t.aux[qi].r = self.nodes[chain[0]].collide;
                }
                Quad::Internal(children) => {
                    let children = *children;
                    let mut r = 0.0;
                    for c in children.into_iter().flatten() {
                        if t.aux[c].r > r {
                            r = t.aux[c].r;
                        }
                    }
                    t.aux[qi].r = r;
                }
            });
            let nodes = &mut self.nodes;
            let random = &mut self.random;
            for i in 0..nodes.len() {
                let ri = nodes[i].collide;
                let ri2 = ri * ri;
                let xi = nodes[i].x + nodes[i].vx;
                let yi = nodes[i].y + nodes[i].vy;
                // d3 applies deltas mid-traversal; later collisions in the
                // same sweep see the updated velocities. Mirror exactly.
                tree.visit(|t, qi, x0, y0, x1, y1| {
                    let rj = t.aux[qi].r;
                    let r = ri + rj;
                    if let Quad::Leaf(chain) = &t.quads[qi] {
                        let j = chain[0];
                        if j > i {
                            let mut x = xi - nodes[j].x - nodes[j].vx;
                            let mut y = yi - nodes[j].y - nodes[j].vy;
                            let mut l = x * x + y * y;
                            if l < r * r {
                                if x == 0.0 {
                                    x = random.jiggle();
                                    l += x * x;
                                }
                                if y == 0.0 {
                                    y = random.jiggle();
                                    l += y * y;
                                }
                                l = l.sqrt();
                                let lk = (r - l) / l; // strength 1
                                let x = x * lk;
                                let y = y * lk;
                                let rj2 = rj * rj;
                                let share = rj2 / (ri2 + rj2);
                                nodes[i].vx += x * share;
                                nodes[i].vy += y * share;
                                nodes[j].vx -= x * (1.0 - share);
                                nodes[j].vy -= y * (1.0 - share);
                            }
                        }
                        return false;
                    }
                    x0 > xi + r || x1 < xi - r || y0 > yi + r || y1 < yi - r
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    /// Bit-level cross-check against real d3-force v3 run in Node
    /// (scratch scenario: pinned core + 5 pinned servers, 3 orgs, 7
    /// leaves incl. a coincident seed exercising the jiggle stream, one
    /// radially parked node; `stop(); tick(300)`). Seeds and expected
    /// coordinates are the JS runtime's exact f64 serializations.
    #[test]
    fn matches_d3_reference_run() {
        const CX: f64 = 800.0;
        const CY: f64 = 450.0;
        struct Spec(&'static str, f64, f64, f64, bool, f64, f64);
        let specs = [
            Spec("core", 800.0, 450.0, 64.0, true, 0.0, 0.0),
            Spec("s0", 800.0, 240.0, 80.0, true, 0.0, 0.0),
            Spec("s1", 999.7218684219822, 385.10643118126103, 80.0, true, 0.0, 0.0),
            Spec("s2", 923.4349029814193, 619.8935688187389, 80.0, true, 0.0, 0.0),
            Spec("s3", 676.5650970185807, 619.8935688187389, 80.0, true, 0.0, 0.0),
            Spec("s4", 600.2781315780178, 385.1064311812611, 80.0, true, 0.0, 0.0),
            Spec("o0", 1069.6796349715262, 673.0983964120414, 52.0, false, 0.0, 0.0),
            Spec("o1", 452.7598545399328, 493.8666317475066, 52.0, false, 0.0, 0.0),
            Spec("o2", 1005.7248383023655, 166.84405196876833, 52.0, false, 0.0, 0.0),
            Spec("t0", 1150.0, 450.0, 30.0, false, 0.0, 0.0),
            Spec("t1", 989.1058070538489, 744.5148446827639, 30.0, false, 0.0, 0.0),
            Spec("t2", 654.3486072085002, 768.2540993889886, 30.0, false, 0.0, 0.0),
            Spec("t3", 453.5026261898441, 499.3920028209535, 30.0, false, 0.0, 0.0),
            Spec("t4", 571.2247326977358, 185.11912664222513, 30.0, false, 0.0, 0.0),
            Spec("t5", 899.2817649121292, 114.37650386790153, 30.0, false, 0.0, 0.0),
            Spec("t6", 452.7598545399328, 493.8666317475066, 30.0, false, 0.0, 0.0),
            Spec("park", 1400.0, 450.0, 40.0, false, 600.0, 0.45),
        ];
        let idx = |id: &str| specs.iter().position(|s| s.0 == id).unwrap();
        let nodes: Vec<SimNode> = specs
            .iter()
            .map(|s| {
                let mut n = SimNode::new(s.1, s.2);
                n.collide = s.3;
                if s.4 {
                    n.fx = Some(s.1);
                    n.fy = Some(s.2);
                }
                n.radial = s.5;
                n.radial_strength = s.6;
                n
            })
            .collect();
        let mut links: Vec<SimLink> = Vec::new();
        for i in 0..3 {
            links.push(SimLink {
                source: idx(&format!("o{i}")),
                target: idx(&format!("s{}", i % 5)),
                dist: 150.0,
                strength: 0.55,
            });
        }
        for i in 0..6 {
            links.push(SimLink {
                source: idx(&format!("t{i}")),
                target: idx(&format!("o{}", i % 3)),
                dist: 56.0,
                strength: 0.6,
            });
            links.push(SimLink {
                source: idx(&format!("t{i}")),
                target: idx(&format!("s{}", i % 5)),
                dist: 100.0,
                strength: 0.35,
            });
        }
        links.push(SimLink { source: idx("t6"), target: idx("o0"), dist: 56.0, strength: 0.6 });
        let mut sim = Simulation::new(nodes, links, CX, CY);
        sim.settle();
        let expected = [
            (800.0, 450.0),
            (800.0, 240.0),
            (999.7218684219822, 385.10643118126103),
            (923.4349029814193, 619.8935688187389),
            (676.5650970185807, 619.8935688187389),
            (600.2781315780178, 385.1064311812611),
            (688.8149945436459, 482.9885024373492),
            (734.2010924859487, 354.4948287269593),
            (870.512416244917, 357.88596185938263),
            (813.4152868330686, 543.0513738734434),
            (754.4047152561069, 532.1999941520313),
            (906.3913405564018, 449.1315594524214),
            (619.0780057738119, 526.1211589846351),
            (672.6444747443568, 300.32119297170277),
            (901.6642504679625, 282.03371586601247),
            (570.450418605901, 490.9735023601805),
            (1384.4155766269369, 450.0954114896141),
        ];
        for (i, (ex, ey)) in expected.iter().enumerate() {
            let n = &sim.nodes[i];
            let dx = (n.x - ex).abs();
            let dy = (n.y - ey).abs();
            assert!(
                dx < 1e-6 && dy < 1e-6,
                "node {} ({}): got ({}, {}), want ({}, {})",
                i,
                specs[i].0,
                n.x,
                n.y,
                ex,
                ey
            );
        }
    }

    #[test]
    fn lcg_matches_js_stream() {
        // First three draws of d3's lcg with s0 = 1.
        let mut r = Lcg::new();
        assert_eq!(r.next(), 1015568748.0 / 4294967296.0);
        let s2 = (1664525.0 * 1015568748.0 + 1013904223.0) % 4294967296.0;
        assert_eq!(r.next(), s2 / 4294967296.0);
        assert!(r.next() > 0.0);
    }

    #[test]
    fn pinned_node_never_moves() {
        let mut a = SimNode::new(100.0, 100.0);
        a.fx = Some(100.0);
        a.fy = Some(100.0);
        let b = SimNode::new(160.0, 100.0);
        let mut sim = Simulation::new(
            vec![a, b],
            vec![SimLink { source: 1, target: 0, dist: 30.0, strength: 0.9 }],
            100.0,
            100.0,
        );
        sim.settle();
        assert_eq!(sim.nodes[0].x, 100.0);
        assert_eq!(sim.nodes[0].y, 100.0);
        assert_ne!(sim.nodes[1].x, 160.0);
    }

    #[test]
    fn link_settles_toward_rest_length() {
        let mut a = SimNode::new(500.0, 450.0);
        a.fx = Some(500.0);
        a.fy = Some(450.0);
        let b = SimNode::new(700.0, 450.0);
        let mut sim = Simulation::new(
            vec![a, b],
            vec![SimLink { source: 1, target: 0, dist: 100.0, strength: 0.9 }],
            500.0,
            450.0,
        );
        sim.settle();
        let dx = sim.nodes[1].x - 500.0;
        let dy = sim.nodes[1].y - 450.0;
        let d = (dx * dx + dy * dy).sqrt();
        // Spring vs charge repulsion equilibrium lands near the rest
        // length; the exact value is the d3 fixed point, not 100.
        assert!(d > 60.0 && d < 220.0, "settled distance {d}");
    }

    #[test]
    fn radial_pulls_to_ring() {
        let mut n = SimNode::new(810.0, 450.0);
        n.radial = 600.0;
        n.radial_strength = 0.45;
        let mut sim = Simulation::new(vec![n], vec![], 800.0, 450.0);
        sim.settle();
        let dx = sim.nodes[0].x - 800.0;
        let dy = sim.nodes[0].y - 450.0;
        let r = (dx * dx + dy * dy).sqrt();
        assert!((r - 600.0).abs() < 60.0, "settled radius {r}");
    }

    #[test]
    fn settle_is_deterministic() {
        let build = || {
            let mut core = SimNode::new(800.0, 450.0);
            core.fx = Some(800.0);
            core.fy = Some(450.0);
            core.collide = 64.0;
            let mut nodes = vec![core];
            let mut links = vec![];
            for i in 0..12 {
                let a = 2.0 * std::f64::consts::PI * (i as f64) / 12.0;
                let mut n = SimNode::new(800.0 + 210.0 * a.cos(), 450.0 + 210.0 * a.sin());
                n.collide = 40.0;
                nodes.push(n);
                links.push(SimLink { source: i + 1, target: 0, dist: 150.0, strength: 0.5 });
            }
            let mut sim = Simulation::new(nodes, links, 800.0, 450.0);
            sim.settle();
            sim.nodes.iter().map(|n| (n.x, n.y)).collect::<Vec<_>>()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn collide_separates_overlapping_nodes() {
        let mut a = SimNode::new(400.0, 400.0);
        a.collide = 30.0;
        let mut b = SimNode::new(410.0, 400.0);
        b.collide = 30.0;
        let mut sim = Simulation::new(vec![a, b], vec![], 405.0, 400.0);
        sim.settle();
        let dx = sim.nodes[1].x - sim.nodes[0].x;
        let dy = sim.nodes[1].y - sim.nodes[0].y;
        let d = (dx * dx + dy * dy).sqrt();
        assert!(d >= 55.0, "separated distance {d}");
    }
}
