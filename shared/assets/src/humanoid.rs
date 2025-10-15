//! Humanoid rig detection (canonical bone names) for retargeting.

use crate::types::SkinnedMeshCPU;

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum HumanoidBone {
    Hips = 0,
    Spine = 1,
    Chest = 2,
    UpperChest = 3,
    Neck = 4,
    Head = 5,
    ClavicleL = 6,
    UpperArmL = 7,
    LowerArmL = 8,
    HandL = 9,
    ClavicleR = 10,
    UpperArmR = 11,
    LowerArmR = 12,
    HandR = 13,
    UpperLegL = 14,
    LowerLegL = 15,
    FootL = 16,
    ToesL = 17,
    UpperLegR = 18,
    LowerLegR = 19,
    FootR = 20,
    ToesR = 21,
}

impl HumanoidBone {
    pub const COUNT: usize = 22;
}

#[derive(Clone, Debug)]
pub struct HumanoidRig {
    pub bone_of_node: Vec<Option<HumanoidBone>>, // node index -> canonical (if any)
    pub node_of_bone: [Option<usize>; HumanoidBone::COUNT], // canonical -> node index
    pub root_node: usize,
}

fn norm_bone_name(s: &str) -> String {
    let mut out = s.to_lowercase();
    for pref in [
        "def-",
        "rig|",
        "rig/",
        "rig:",
        "armature|",
        "armature/",
        "armature:",
        "skeleton|",
        "skeleton/",
        "skeleton:",
        "mixamorig:",
    ] {
        out = out.replace(pref, "");
    }
    out = out.replace([' ', '_', '-', '.', '|'], "");
    out = out
        .replace("hips", "pelvis")
        .replace("forearm", "lowerarm")
        .replace("shoulder", "clavicle")
        .replace("shin", "calf");
    out
}

fn match_bone(n: &str, is_left: bool, is_right: bool) -> Option<HumanoidBone> {
    let l = norm_bone_name(n);
    macro_rules! has {
        ($k:expr) => {
            l.contains($k)
        };
    }
    let left = is_left || l.contains("left") || (l.ends_with('l') && !l.contains("leg"));
    let right = is_right || l.contains("right") || (l.ends_with('r') && !l.contains("leg"));

    if has!("pelvis") || (has!("hip") && !has!("upper")) {
        return Some(HumanoidBone::Hips);
    }
    if has!("spine2") || has!("upperchest") {
        return Some(HumanoidBone::UpperChest);
    }
    if has!("spine1") || has!("chest") {
        return Some(HumanoidBone::Chest);
    }
    if has!("spine") {
        return Some(HumanoidBone::Spine);
    }
    if has!("neck") {
        return Some(HumanoidBone::Neck);
    }
    if has!("head") {
        return Some(HumanoidBone::Head);
    }

    if has!("clavicle") && left {
        return Some(HumanoidBone::ClavicleL);
    }
    if has!("clavicle") && right {
        return Some(HumanoidBone::ClavicleR);
    }
    if has!("upperarm") && left {
        return Some(HumanoidBone::UpperArmL);
    }
    if has!("upperarm") && right {
        return Some(HumanoidBone::UpperArmR);
    }
    if has!("lowerarm") && left {
        return Some(HumanoidBone::LowerArmL);
    }
    if has!("lowerarm") && right {
        return Some(HumanoidBone::LowerArmR);
    }
    if has!("hand") && left {
        return Some(HumanoidBone::HandL);
    }
    if has!("hand") && right {
        return Some(HumanoidBone::HandR);
    }

    if (has!("upperleg") || (has!("thigh") && !has!("lower"))) && left {
        return Some(HumanoidBone::UpperLegL);
    }
    if (has!("upperleg") || (has!("thigh") && !has!("lower"))) && right {
        return Some(HumanoidBone::UpperLegR);
    }
    if (has!("lowerleg") || has!("calf")) && left {
        return Some(HumanoidBone::LowerLegL);
    }
    if (has!("lowerleg") || has!("calf")) && right {
        return Some(HumanoidBone::LowerLegR);
    }
    if has!("foot") && left {
        return Some(HumanoidBone::FootL);
    }
    if has!("foot") && right {
        return Some(HumanoidBone::FootR);
    }
    if (has!("toe") || has!("ball")) && left {
        return Some(HumanoidBone::ToesL);
    }
    if (has!("toe") || has!("ball")) && right {
        return Some(HumanoidBone::ToesR);
    }

    None
}

pub fn detect_humanoid(sk: &SkinnedMeshCPU) -> HumanoidRig {
    let mut bone_of_node = vec![None; sk.parent.len()];
    let mut node_of_bone: [Option<usize>; HumanoidBone::COUNT] = [None; HumanoidBone::COUNT];

    for (i, name) in sk.node_names.iter().enumerate() {
        let is_left = name.contains(".L") || name.contains("_L");
        let is_right = name.contains(".R") || name.contains("_R");
        if let Some(b) = match_bone(name.as_str(), is_left, is_right)
            && node_of_bone[b as usize].is_none()
        {
            bone_of_node[i] = Some(b);
            node_of_bone[b as usize] = Some(i);
        }
    }
    let root_node = node_of_bone[HumanoidBone::Hips as usize]
        .or(sk.root_node)
        .unwrap_or(0);

    HumanoidRig {
        bone_of_node,
        node_of_bone,
        root_node,
    }
}
