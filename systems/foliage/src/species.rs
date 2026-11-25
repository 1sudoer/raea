//! The species (docs/foliage.feature FO4): each a row of numbers. Leaf sizes and crown habits follow the floras'
//! descriptions; the growth numbers (step, reach, kill, points, tropism) are tuned by eye against photographs and are
//! ASSUMED until a species' sheet beside its photographs says otherwise (FO12).

/// A leaf's outline rules (atlas.rs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeafKind {
    /// heart-shaped, oblique, finely toothed: lime
    Cordate,
    /// egg-shaped, lopsided base, double-toothed: elm
    Ovate,
    /// long and narrow: willow
    Lanceolate,
    /// broad-based, pointed, double-toothed: birch
    Triangular,
    /// diamond: poplar
    Rhombic,
    /// rounded lobes, small ears at the base: oak
    Oak,
    /// five pointed lobes with few big teeth: Norway maple
    Palmate,
    /// three to five broad lobes, coarse teeth: plane
    Plane,
    /// five to seven leaflets from one point: horse chestnut
    Compound,
    /// leaflets in pairs along a stalk: black locust
    Pinnate,
}

#[derive(Clone, Debug)]
pub struct Species {
    pub id: &'static str,
    pub name: &'static str,
    /// the crown: where it's widest (0 its base, 1 its top), its squareness (2 an ellipse), the leafy shell's depth
    /// (of the radius), how lumpy it is, height over width when only a width is known (the median of Berlin's trees
    /// of 50 years or more in the Baumbestand, dl-de/zero-2.0; the poplar's is a Lombardy's, ASSUMED), trunk radius a
    /// metre of height
    pub widest: f32,
    pub square: f32,
    /// the squareness below the widest point (1 a cone, an elm's vase; 3 a skirt, a chestnut's)
    pub low: f32,
    pub shell: f32,
    pub lumps: f32,
    pub height_per_width: f32,
    pub trunk: f32,
    /// growth: step (m), reach and kill (in steps), attraction points a cubic metre of shell, the share of points
    /// kept inside the shell, tropism up (+) or down (-), droop of thin shoots (radians a metre at a twig's radius),
    /// a tip's radius (the pipe model is fitted to it and the trunk), twigs sprouted a tip and their length (m)
    pub step: f32,
    pub reach: f32,
    pub kill: f32,
    pub points: f32,
    pub interior: f32,
    pub up: f32,
    pub droop: f32,
    pub tip: f32,
    pub twigs: f32,
    pub twig_len: f32,
    /// leaves: kind, length (m, with its stalk), width over length, leaf area index (leaf area over the crown's ground area; ASSUMED typical values), how far back from a
    /// tip shoots carry leaves (m), how much a leaf hangs
    pub leaf: LeafKind,
    pub leaf_len: f32,
    pub aspect: f32,
    pub lai: f32,
    pub leafy: f32,
    pub leaf_droop: f32,
    /// leaves drawn on one card (a sprig: a twig with its leaves), and whether they grow in opposite pairs
    pub sprig: u32,
    pub opposite: bool,
    /// how much of the crown is sky holes between clumps of foliage (ASSUMED by eye)
    pub gaps: f32,
    /// architecture: how far up the crown the trunk carries on as a leader (0 to 1), and how many big limbs it forks
    /// into there (0: none; elms, oaks and willows fork, poplars and birches keep a leader)
    pub leader: f32,
    pub forks: u32,
    /// the crown's masses: how many big lobes it is (each on a scaffold limb from the trunk, for a 10 m crown; more
    /// on a bigger one), their radius (of the crown's), how crooked the limbs grow (a step's random turn) and how far
    /// the crown leans off its trunk (of its radius): a real crown is a few clouds with sky between, not one ball
    pub lobes: f32,
    pub lobe: f32,
    pub crook: f32,
    pub skew: f32,
    /// bark: linear colour, and its pattern (0 fissured, 1 a plane's flakes, 2 a birch's white, 3 a locust's ropes)
    pub bark: [f32; 3],
    pub bark_kind: u32,
    /// the foot (roots.rs): the trunk's flare, its diameter at the ground over its diameter at breast height (1.3-1.9
    /// on urban trees, North 2015 and Hilbert 2020 as cited by Lee 2022, AUF 48(4)); its structural roots (5-15,
    /// Day et al. 2010 after Wilson 1964); and how much of a root's height shows above the soil by the trunk (0 none,
    /// 1 all), higher for the species known for surface roots (maple, plane, elm and willow, WSU Extension C137; birch,
    /// Forest Research) and lower for oak and lime, which C137 names as good choices. Each species' values ASSUMED
    /// inside those ranges.
    pub flare: f32,
    pub roots: u32,
    pub surface: f32,
}

const fn sp(id: &'static str, name: &'static str) -> Species {
    Species {
        id,
        name,
        widest: 0.45,
        square: 2.2,
        low: 2.2,
        shell: 0.38,
        lumps: 0.15,
        height_per_width: 1.4,
        trunk: 0.021,
        step: 0.24,
        reach: 13.0,
        kill: 2.5,
        points: 20.0,
        interior: 0.35,
        up: 0.3,
        droop: 0.08,
       
        tip: 0.003,
        twigs: 2.0,
        twig_len: 0.45,
        leaf: LeafKind::Cordate,
        leaf_len: 0.09,
        aspect: 1.0,
        lai: 5.0,
        leafy: 0.9,
        leaf_droop: 0.1,
        sprig: 5,
        opposite: false,
        gaps: 0.3,
        leader: 0.4,
        forks: 0,
        lobes: 7.0,
        lobe: 0.5,
        crook: 0.12,
        skew: 0.08,
        bark: [0.18, 0.16, 0.14],
        bark_kind: 0,
        flare: 1.5,
        roots: 7,
        surface: 0.3,
    }
}

pub static ALL: [Species; 10] = [
    // small-leaved lime: a dense ovoid of ascending limbs, red-brown twigs, leaves 3-10 cm, heart-shaped and oblique
    Species { widest: 0.42, square: 1.7, low: 2.0, lumps: 0.2, height_per_width: 1.39, trunk: 0.02, step: 0.22, reach: 14.0, kill: 2.6, points: 22.5, up: 0.35, droop: 0.05, twigs: 2.5, leaf_len: 0.085, aspect: 0.95, lai: 7.0, sprig: 7, leader: 0.55, forks: 0, lobes: 13.0, lobe: 0.4, crook: 0.1, skew: 0.06, gaps: 0.12, leafy: 1.4, flare: 1.5, roots: 9, surface: 0.2, ..sp("lime", "Tilia cordata") },
    // field and fluttering elm: a broad vase, widest high, leaves 6-12 cm, lopsided at the base
    Species { widest: 0.62, square: 2.4, low: 1.1, shell: 0.32, lumps: 0.2, height_per_width: 1.92, trunk: 0.022, step: 0.25, reach: 14.0, kill: 2.4, points: 17.5, up: 0.55, droop: 0.12, twigs: 2.2, twig_len: 0.5, leaf: LeafKind::Ovate, leaf_len: 0.1, aspect: 0.62, lai: 5.0, leafy: 1.2, sprig: 7, leader: 0.15, forks: 3, gaps: 0.2, bark: [0.17, 0.15, 0.13], lobes: 6.0, lobe: 0.52, crook: 0.16, skew: 0.12, flare: 1.8, roots: 7, surface: 0.45, ..sp("elm", "Ulmus minor") },
    // London plane: a broad dome on big limbs, 20-30 m by 15-20 m (berlin.de), leaves 12-25 cm, flaking bark
    Species { widest: 0.45, square: 2.6, low: 2.2, shell: 0.35, lumps: 0.18, height_per_width: 1.15, trunk: 0.025, step: 0.3, reach: 13.0, kill: 2.5, points: 15.0, up: 0.2, droop: 0.08, twigs: 1.6, twig_len: 0.5, leaf: LeafKind::Plane, leaf_len: 0.2, aspect: 1.1, lai: 5.5, leafy: 1.3, sprig: 2, leader: 0.3, forks: 3, gaps: 0.12, bark: [0.42, 0.40, 0.32], bark_kind: 1, lobes: 6.0, lobe: 0.52, crook: 0.22, skew: 0.12, flare: 1.7, roots: 8, surface: 0.45, ..sp("plane", "Platanus x acerifolia") },
    // Norway maple: round and dense, leaves 10-15 cm with five to seven pointed lobes
    Species { widest: 0.45, square: 2.4, low: 2.4, shell: 0.4, lumps: 0.1, height_per_width: 1.42, trunk: 0.021, step: 0.22, reach: 13.0, kill: 2.5, points: 22.5, up: 0.3, droop: 0.05, twigs: 2.0, twig_len: 0.4, leaf: LeafKind::Palmate, leaf_len: 0.13, aspect: 1.15, lai: 6.0, leafy: 1.3, sprig: 4, leader: 0.35, forks: 0, gaps: 0.1, opposite: true, bark: [0.22, 0.21, 0.19], lobes: 11.0, lobe: 0.43, crook: 0.12, skew: 0.07, flare: 1.6, roots: 8, surface: 0.45, ..sp("maple", "Acer platanoides") },
    // horse chestnut: a broad dome, lower limbs sweeping down and up at the tips, leaves of 5-7 leaflets 10-25 cm
    Species { widest: 0.4, square: 2.2, low: 3.0, shell: 0.42, lumps: 0.12, height_per_width: 1.27, trunk: 0.024, step: 0.25, reach: 13.0, kill: 2.5, points: 20.0, up: 0.05, droop: 0.35, twigs: 1.4, twig_len: 0.35, leaf: LeafKind::Compound, leaf_len: 0.32, aspect: 1.1, lai: 7.0, leafy: 1.1, sprig: 1, leader: 0.3, forks: 0, gaps: 0.06, leaf_droop: 0.35, bark: [0.25, 0.22, 0.19], lobes: 9.0, lobe: 0.46, crook: 0.12, skew: 0.08, flare: 1.5, roots: 7, surface: 0.3, ..sp("chestnut", "Aesculus hippocastanum") },
    // black locust: open and irregular, leaves 15-30 cm of 9-19 leaflets, deep rope-like bark
    Species { widest: 0.55, square: 1.8, low: 1.6, shell: 0.3, lumps: 0.3, height_per_width: 1.89, trunk: 0.02, step: 0.3, reach: 12.0, kill: 2.2, points: 12.5, up: 0.4, droop: 0.08, twigs: 1.2, twig_len: 0.5, leaf: LeafKind::Pinnate, leaf_len: 0.25, aspect: 0.6, lai: 3.0, leafy: 1.0, sprig: 2, leader: 0.3, forks: 2, gaps: 0.35, leaf_droop: 0.25, bark: [0.27, 0.24, 0.2], bark_kind: 3, lobes: 5.0, lobe: 0.5, crook: 0.3, skew: 0.16, flare: 1.4, roots: 6, surface: 0.3, ..sp("locust", "Robinia pseudoacacia") },
    // pedunculate oak: broad, on crooked limbs, leaves 7-14 cm with rounded lobes
    Species { widest: 0.5, square: 3.2, low: 2.0, shell: 0.32, lumps: 0.3, height_per_width: 1.5, trunk: 0.028, step: 0.3, reach: 12.0, kill: 2.3, points: 15.0, up: 0.15, droop: 0.05, twigs: 1.8, twig_len: 0.4, leaf: LeafKind::Oak, leaf_len: 0.11, aspect: 0.62, lai: 5.0, leafy: 1.0, sprig: 6, leader: 0.15, forks: 4, gaps: 0.32, bark: [0.2, 0.18, 0.15], lobes: 6.0, lobe: 0.5, crook: 0.35, skew: 0.15, flare: 1.6, roots: 6, surface: 0.2, ..sp("oak", "Quercus robur") },
    // silver birch: narrow, its twigs hanging, leaves 3-7 cm, white bark
    Species { widest: 0.4, square: 1.6, low: 1.6, shell: 0.35, height_per_width: 1.71, trunk: 0.014, step: 0.2, reach: 14.0, kill: 2.6, points: 17.5, up: 0.45, droop: 0.45, twigs: 3.0, twig_len: 0.6, leaf: LeafKind::Triangular, leaf_len: 0.055, aspect: 0.75, lai: 3.5, leafy: 1.2, sprig: 9, leader: 0.75, forks: 0, gaps: 0.28, leaf_droop: 0.2, bark: [0.75, 0.74, 0.7], bark_kind: 2, lobes: 9.0, lobe: 0.42, crook: 0.12, skew: 0.1, flare: 1.3, roots: 7, surface: 0.4, ..sp("birch", "Betula pendula") },
    // Lombardy poplar: a column, leaves 5-10 cm, diamond-shaped
    Species { widest: 0.3, square: 1.4, low: 1.5, shell: 0.5, lumps: 0.08, height_per_width: 4.5, trunk: 0.018, step: 0.22, reach: 10.0, kill: 2.4, points: 25.0, up: 2.4, droop: 0.0, twigs: 2.0, twig_len: 0.35, leaf: LeafKind::Rhombic, leaf_len: 0.07, aspect: 0.9, lai: 3.5, leafy: 0.8, sprig: 7, leader: 0.85, forks: 0, gaps: 0.12, bark: [0.24, 0.23, 0.2], lobes: 8.0, lobe: 0.55, crook: 0.05, skew: 0.03, flare: 1.6, roots: 8, surface: 0.4, ..sp("poplar", "Populus nigra 'Italica'") },
    // white willow: broad, its twigs hanging by the water, leaves 5-12 cm, narrow
    Species { widest: 0.5, square: 2.4, low: 1.4, shell: 0.4, lumps: 0.2, height_per_width: 1.19, trunk: 0.03, step: 0.25, reach: 13.0, kill: 2.4, points: 17.5, up: 0.3, droop: 0.7, twigs: 3.0, twig_len: 0.8, leaf: LeafKind::Lanceolate, leaf_len: 0.09, aspect: 0.3, lai: 3.5, leafy: 1.6, sprig: 9, leader: 0.1, forks: 3, leaf_droop: 0.3, bark: [0.24, 0.22, 0.19], lobes: 6.0, lobe: 0.5, crook: 0.2, skew: 0.12, gaps: 0.22, flare: 1.7, roots: 9, surface: 0.5, ..sp("willow", "Salix alba") },
];

pub fn species(id: &str) -> Option<&'static Species> {
    ALL.iter().find(|s| s.id == id)
}
