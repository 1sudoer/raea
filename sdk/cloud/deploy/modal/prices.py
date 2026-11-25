# Modal's list prices (modal.com/pricing, read 2026-11-17), in dollars per second, and what a container cost. A GPU
# container also pays for its CPU (per physical core, two vCPUs) and memory, which on an L4 is most of the price again
# at 8 cores and 32 GiB: why bench/gpus.py's numbers come with the container's size.

import re

GPU = {
    "B300": 0.001972,
    "B200": 0.001736,
    "H200": 0.001261,
    "H100": 0.001097,
    "RTX-PRO-6000": 0.000842,
    "A100-80GB": 0.000694,
    "A100-40GB": 0.000583,
    "L40S": 0.000542,
    "A10": 0.000306,
    "L4": 0.000222,
    "T4": 0.000164,
}
CORE = 0.0000131  # one physical core
GIB = 0.00000222  # one GiB of memory


def per_hour(kind, cpu, memory_gib):
    return round(3600 * (GPU.get(kind, 0) + cpu * CORE + memory_gib * GIB), 4)


def cost(kind, cpu, memory_gib, seconds):
    return round(seconds * per_hour(kind, cpu, memory_gib) / 3600, 4)


def kind_of(name):
    """nvidia-smi's name for a GPU (NVIDIA L40S, Tesla T4, NVIDIA RTX PRO 6000 Blackwell Server Edition, NVIDIA
    A100-SXM4-40GB) as its name here, or "" when it isn't one."""
    n = name.upper()
    if "RTX PRO 6000" in n:
        return "RTX-PRO-6000"
    if "A100" in n:
        return "A100-80GB" if "80GB" in n else "A100-40GB"
    for k in ("B300", "B200", "H200", "H100", "L40S", "A10", "L4", "T4"):
        if re.search(rf"\b{k}\b", n):
            return k
    return ""
