import re, pathlib, sys

# Usage: extract.py [path/to/FSRCNNX.hlsl]
# Defaults to a Magpie checkout next to the rustdesk checkout.
_here = pathlib.Path(__file__).resolve().parent
_default = _here / '..' / '..' / '..' / '..' / 'Magpie' / 'src' / 'Effects' / 'FSRCNNX' / 'FSRCNNX.hlsl'
src = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else _default).read_text()

def floats(s):
    return [float(x) for x in re.findall(r'[-+]?\d*\.\d+(?:[eE][-+]?\d+)?|[-+]?\d+\.\d*', s)]

# Split passes
chunks = re.split(r'//!PASS\s+(\d+)', src)
# chunks[0] preamble, then pairs (num, body)
passes = {}
for i in range(1, len(chunks), 2):
    passes[int(chunks[i])] = chunks[i+1]

out_lines = []
out_lines.append('// Generated from Magpie FSRCNNX.hlsl (FSRCNNX_x2_8-0-4-1 general variant).')
out_lines.append('// Do not edit by hand. See libs/upscaler/tools/extract.py convention.')
out_lines.append('// Layout notes: HLSL mul(rowvec,mat) with row-major MF4x4 literals;')
out_lines.append('// MulAdd(x,M,a)[j] = a[j] + sum_i x[i]*M[i][j].')

def parse_mf4_list(body):
    ms = re.findall(r'MF4\s*\(([^)]+)\)', body)
    return [floats(m) for m in ms]

def parse_brace4_list(body):
    ms = re.findall(r'MF4\s+\w+\s*=\s*\{([^}]+)\}', body)
    return [floats(m) for m in ms]

def parse_muladds(body):
    # returns list of (varname, 16 floats) in order
    ms = re.findall(r'MulAdd\s*\(\s*(\w+)\s*,\s*MF4x4\s*\(([^)]+)\)', body)
    return [(name, floats(vals)) for name, vals in ms]

def parse_mads(body):
    ms = re.findall(r'mad\s*\(\s*(src\[\d\]\[\d\])\s*,\s*MF4\s*\(([^)]+)\)', body)
    return [(name, floats(vals)) for name, vals in ms]

# ---- Pass 1 ----
b1 = passes[1]
mads = parse_mads(b1)
assert len(mads) == 50, len(mads)
mf4s = parse_mf4_list(b1)
# mf4s includes GetLuma MF3? No, MF4 only. First two MF4 are biases? Check: MF4(-0.157...) target1 bias, then 50 mad MF4s, then MF4(0.054...) target2 bias... Actually order: bias1, 25 mad weights, bias2, 25 mad weights.
# So mf4 list = [bias1, 25 w, bias2, 25 w] = 52 total. Good.
assert len(mf4s) == 52, len(mf4s)
b1_t1 = mf4s[0]
w1_t1 = mf4s[1:26]
b1_t2 = mf4s[26]
w1_t2 = mf4s[27:52]
# tap order check against mad var names
taps = [n for n,_ in mads]
# first 25 should be src[a][b] sequence, next 25 same
print('P1 taps head:', taps[:6], 'mid:', taps[24:27])

# Build P1_BIAS[8], P1_W[8][25]
p1_bias = b1_t1 + b1_t2
p1_w = []  # out 0..7
for o in range(4):
    p1_w.append([w[o] for w in w1_t1])
for o in range(4):
    p1_w.append([w[o] for w in w1_t2])

# emit with f32 suffix
def rust_f32_list(vals):
    return '[' + ', '.join(f'{v!r}f32' for v in vals) + ']'
out_lines.append(f'pub const P1_BIAS: [f32; 8] = {rust_f32_list(p1_bias)};')
out_lines.append(f'pub const P1_W: [[f32; 25]; 8] = [')
for o in range(8):
    out_lines.append(f'    {rust_f32_list(p1_w[o])},')
out_lines.append('];')
# tap dx/dy for tap index k (file order x-major: dx=k//5-2, dy=k%5-2)
out_lines.append('pub const P1_TAP_DX: [i32; 25] = [%s];' % ', '.join(str(k//5-2) for k in range(25)))
out_lines.append('pub const P1_TAP_DY: [i32; 25] = [%s];' % ', '.join(str(k%5-2) for k in range(25)))

SPATIAL = ['tl','ml','bl','tc','mc','bc','tr','mr','br']
SDX = {'tl':-1,'ml':-1,'bl':-1,'tc':0,'mc':0,'bc':0,'tr':1,'mr':1,'br':1}
SDY = {'tl':-1,'ml':0,'bl':1,'tc':-1,'mc':0,'bc':1,'tr':-1,'mr':0,'br':1}

def build_conv3x3(body, name):
    muls = parse_muladds(body)
    assert len(muls) == 36, (name, len(muls))
    mf4 = parse_mf4_list(body)
    # mf4 = [bias1, slope1, bias2, slope2] in order? Check: bias MF4, ...muls..., slope MF4, bias MF4, ...muls..., slope MF4
    assert len(mf4) == 4, (name, len(mf4))
    bias1, slope1, bias2, slope2 = mf4
    # split muls: first 18 -> target1, next 18 -> target2
    g1 = muls[:18]; g2 = muls[18:]
    # within each group order: tl1,tl2,ml1,ml2,...,br1,br2
    def group_to_mats(g):
        mats = {}  # (s, half) -> 4x4 rows
        for idx in range(9):
            n1, m1 = g[idx*2]; n2, m2 = g[idx*2+1]
            s = SPATIAL[idx]
            assert n1 == s+'1' and n2 == s+'2', (name, n1, n2)
            # 16 floats row-major -> rows[i][j]
            mats[(s,1)] = [m1[i*4:(i+1)*4] for i in range(4)]
            mats[(s,2)] = [m2[i*4:(i+1)*4] for i in range(4)]
        return mats
    m1 = group_to_mats(g1); m2 = group_to_mats(g2)
    # w[out][tap][in]
    import copy
    w = [[[0.0]*8 for _ in range(9)] for _ in range(8)]
    for s_idx, s in enumerate(SPATIAL):
        A1 = m1[(s,1)]; B1 = m1[(s,2)]
        for j in range(4):
            for i in range(4):
                w[j][s_idx][i] = A1[i][j]
                w[j][s_idx][4+i] = B1[i][j]
        A2 = m2[(s,1)]; B2 = m2[(s,2)]
        for j in range(4):
            for i in range(4):
                w[4+j][s_idx][i] = A2[i][j]
                w[4+j][s_idx][4+i] = B2[i][j]
    bias = bias1 + bias2
    slope = slope1 + slope2
    return bias, slope, w

for pn in (2,3,4):
    bias, slope, w = build_conv3x3(passes[pn], f'P{pn}')
    out_lines.append(f'pub const P{pn}_BIAS: [f32; 8] = {rust_f32_list(bias)};')
    out_lines.append(f'pub const P{pn}_SLOPE: [f32; 8] = {rust_f32_list(slope)};')
    out_lines.append(f'pub const P{pn}_W: [[[f32; 8]; 9]; 8] = [')
    for o in range(8):
        out_lines.append(f'    [ // out{o}')
        for s in range(9):
            out_lines.append(f'        {rust_f32_list(w[o][s])}, // {SPATIAL[s]}')
        out_lines.append('    ],')
    out_lines.append('];')

# ---- Pass 5 ----
b5 = passes[5]
muls5 = parse_muladds(b5)
assert len(muls5) == 40, len(muls5)
mf4_5 = parse_mf4_list(b5)
brace5 = parse_brace4_list(b5)
print('P5 mf4:', len(mf4_5), 'brace:', len(brace5))
# mf4_5 = [slope_c1, slope_c2, bias_t1, slope_t1, bias_t2, slope_t2] (6)
# brace5 = [bias_c1{...}, bias_c2{...}]? plus? brace regex MF4 c1 = {...} -> 2, plus target1 MF4(...) parens not braces. Also P6? no.
assert len(mf4_5) == 6 and len(brace5) == 2, (len(mf4_5), len(brace5))
slope_c1, slope_c2, bias_t1, slope_t1, bias_t2, slope_t2 = mf4_5
bias_c1, bias_c2 = brace5
g_c1 = muls5[:18]; g_c2 = muls5[18:36]
m_1x1_t1 = muls5[36]; m_1x1_t2 = muls5[37]
assert m_1x1_t1[0] == 'c1' and m_1x1_t2[0] == 'c1' or True
# Actually lines: MulAdd(c1,...target1), MulAdd(c2,...target1), then target2: MulAdd(c1..), MulAdd(c2..)
# parse: muls5[36]=(c1,...), [37]=(c2,...), [38]=(c1,...), [39]=(c2,...)
names = [n for n,_ in muls5[36:]]
assert names == ['c1','c2','c1','c2'], names

def group_to_mats5(g):
    mats = {}
    for idx in range(9):
        n1, m1v = g[idx*2]; n2, m2v = g[idx*2+1]
        s = SPATIAL[idx]
        mats[(s,1)] = [m1v[i*4:(i+1)*4] for i in range(4)]
        mats[(s,2)] = [m2v[i*4:(i+1)*4] for i in range(4)]
    return mats
mc1 = group_to_mats5(g_c1); mc2 = group_to_mats5(g_c2)
w_c1 = [[[0.0]*8 for _ in range(9)] for _ in range(8)]
for s_idx, s in enumerate(SPATIAL):
    A1 = mc1[(s,1)]; B1 = mc1[(s,2)]
    for j in range(4):
        for i in range(4):
            w_c1[j][s_idx][i] = A1[i][j]
            w_c1[j][s_idx][4+i] = B1[i][j]
    A2 = mc2[(s,1)]; B2 = mc2[(s,2)]
    for j in range(4):
        for i in range(4):
            w_c1[4+j][s_idx][i] = A2[i][j]
            w_c1[4+j][s_idx][4+i] = B2[i][j]
bias_c = bias_c1 + bias_c2
slope_c = slope_c1 + slope_c2
out_lines.append(f'pub const P5_C_BIAS: [f32; 8] = {rust_f32_list(bias_c)};')
out_lines.append(f'pub const P5_C_SLOPE: [f32; 8] = {rust_f32_list(slope_c)};')
out_lines.append(f'pub const P5_C_W: [[[f32; 8]; 9]; 8] = [')
for o in range(8):
    out_lines.append('    [')
    for s in range(9):
        out_lines.append(f'        {rust_f32_list(w_c1[o][s])},')
    out_lines.append('    ],')
out_lines.append('];')
# 1x1: Ma (c1->t1 4x4), Mb (c2->t1), Mc (c1->t2), Md (c2->t2), each 16 floats row-major
Ma = muls5[36][1]; Mb = muls5[37][1]; Mc = muls5[38][1]; Md = muls5[39][1]
def rows(v): return [v[i*4:(i+1)*4] for i in range(4)]
Ra, Rb, Rc, Rd = rows(Ma), rows(Mb), rows(Mc), rows(Md)
w11 = [[0.0]*8 for _ in range(8)]  # [out][in]
for j in range(4):
    for i in range(4):
        w11[j][i] = Ra[i][j]
        w11[j][4+i] = Rb[i][j]
        w11[4+j][i] = Rc[i][j]
        w11[4+j][4+i] = Rd[i][j]
out_lines.append(f'pub const P5_T_BIAS: [f32; 8] = {rust_f32_list(bias_t1 + bias_t2)};')
out_lines.append(f'pub const P5_T_SLOPE: [f32; 8] = {rust_f32_list(slope_t1 + slope_t2)};')
out_lines.append(f'pub const P5_T_W: [[f32; 8]; 8] = [')
for o in range(8):
    out_lines.append(f'    {rust_f32_list(w11[o])},')
out_lines.append('];')

# ---- Pass 6 ----
b6 = passes[6]
muls6 = parse_muladds(b6)
assert len(muls6) == 18, len(muls6)
brace6 = parse_brace4_list(b6)
print('P6 brace:', brace6)
assert len(brace6) == 1
bias6 = brace6[0]
assert len(bias6) == 4
w6 = [[[0.0]*8 for _ in range(9)] for _ in range(4)]
for idx in range(9):
    n1, m1v = muls6[idx*2]; n2, m2v = muls6[idx*2+1]
    A = [m1v[i*4:(i+1)*4] for i in range(4)]
    B = [m2v[i*4:(i+1)*4] for i in range(4)]
    for j in range(4):
        for i in range(4):
            w6[j][idx][i] = A[i][j]
            w6[j][idx][4+i] = B[i][j]
out_lines.append(f'pub const P6_BIAS: [f32; 4] = {rust_f32_list(bias6)};')
out_lines.append(f'pub const P6_W: [[[f32; 8]; 9]; 4] = [')
for o in range(4):
    out_lines.append('    [')
    for s in range(9):
        out_lines.append(f'        {rust_f32_list(w6[o][s])},')
    out_lines.append('    ],')
out_lines.append('];')

(_here / '..' / 'src' / 'weights.rs').resolve().write_text('\n'.join(out_lines) + '\n')
print('wrote', len('\n'.join(out_lines)), 'chars')
