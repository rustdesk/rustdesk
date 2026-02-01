Texture2D g_txFrame0 : register(t0);
Texture2D g_txFrame1 : register(t1);
SamplerState g_Sam : register(s0);

struct PS_INPUT
{
    float4 Pos : SV_POSITION;
    float2 Tex : TEXCOORD0;
};
float4 PS(PS_INPUT input) : SV_TARGET{
  float y = g_txFrame0.Sample(g_Sam, input.Tex).r;
  y = 1.164383561643836 * (y - 0.0625);
  float2 uv = g_txFrame1.Sample(g_Sam, input.Tex).rg - float2(0.5f, 0.5f);
  float u = uv.x;
  float v = uv.y;
  float r = saturate(y + 1.596026785714286 * v);
  float g = saturate(y - 0.812967647237771 * v - 0.391762290094914 * u);
  float b = saturate(y + 2.017232142857142 * u);
  return float4(r, g, b, 1.0f);
}