// Original showcase shaders. Output matches GPUI's premultiplied UNORM sprites.
cbuffer Frame : register(b0) {
    float4 clock; // time, width, height, mode
    float4 camera; // yaw, pitch, zoom, motion intensity
};
static const float PI = 3.14159265359;
float hash(float x) { return frac(sin(x * 127.1) * 43758.5453); }
float2 rotate(float2 p, float a) { float s=sin(a), c=cos(a); return float2(c*p.x-s*p.y,s*p.x+c*p.y); }
struct Screen { float4 position : SV_Position; float2 uv : TEXCOORD0; };
Screen ScreenVS(uint id : SV_VertexID) {
    Screen o; o.uv=float2((id << 1) & 2, id & 2);
    o.position=float4(o.uv*float2(2,-2)+float2(-1,1),0,1); return o;
}
float4 FieldPS(Screen i) : SV_Target {
    float2 uv=i.uv, p=(uv-.5)*float2(clock.y/clock.z,1);
    float t=clock.x*.24;
    float3 color=float3(.026,.041,.067);
    float2 q=rotate(p,t*.25);
    float v=0;
    [unroll] for(int k=1;k<=5;k++) {
        q+=.14*sin(q.yx*(float)k*2.4+t+float2(k,k*1.7));
        v+=sin(q.x*5+q.y*3+t*1.2)/(float)k;
    }
    float band=pow(.5+.5*sin(v*2.6+p.y*3),3);
    float glow=exp(-2.1*dot(p,p));
    float3 tint=lerp(float3(.12,.27,.53),float3(.75,.94,.46),smoothstep(-.35,.8,q.y+sin(t)*.25));
    color+=tint*band*glow*.85;
    float contour=pow(.5+.5*cos(v*17+p.x*5),28)*.1;
    color+=tint*contour*glow;
    color+=float3(.33,.52,.61)*pow(saturate(1-length(p-float2(.05,-.1))),5)*.17;
    color+=(hash(dot(i.position.xy,float2(12.4,37.7)))-.5)/255;
    return float4(color,1);
}
Texture2D Skin : register(t0);
SamplerState SkinSampler : register(s0);
struct MeshIn { float3 position : POSITION; float3 normal : NORMAL; float part : TEXCOORD0; float2 uv : TEXCOORD1; };
struct MeshOut { float4 position : SV_Position; float3 world : TEXCOORD0; float3 normal : TEXCOORD1; nointerpolation float part : TEXCOORD2; float3 local : TEXCOORD3; float2 uv : TEXCOORD4; };
MeshOut ModelVS(MeshIn i) {
    MeshOut o; float3 p=i.position,n=i.normal;
    float part=fmod(i.part,8);
    if(part>=2 && part<=5) {
        float pivot=part<=3 ? 1.68 : .84;
        float phase=(part==2 || part==5) ? 0 : PI;
        float a=sin(clock.x*2.2+phase)*.2*camera.w;
        p.yz=rotate(p.yz-float2(pivot,0),a)+float2(pivot,0); n.yz=rotate(n.yz,a);
    }
    o.local=i.position; o.world=p; o.part=i.part; o.uv=i.uv;
    p.y-=1.12;
    p.xz=rotate(p.xz,camera.x); n.xz=rotate(n.xz,camera.x);
    p.yz=rotate(p.yz,camera.y); n.yz=rotate(n.yz,camera.y);
    p.z+=4.6/camera.z;
    float f=2.6, nearZ=.1, farZ=40;
    o.position=float4(p.x*f/(clock.y/clock.z),p.y*f,farZ/(farZ-nearZ)*p.z-nearZ*farZ/(farZ-nearZ),p.z);
    o.normal=n; return o;
}
float4 ModelPS(MeshOut i) : SV_Target {
    float3 n=normalize(i.normal);
    if(i.part==6) {
        float2 grid=abs(frac(i.world.xz*3-.5)-.5)/max(fwidth(i.world.xz*3),.0001);
        float gridLine=1-min(min(grid.x,grid.y),1);
        float fade=saturate(1-length(i.world.xz)/2.5);
        float shadow=1-.6*exp(-dot(i.world.xz,i.world.xz)*3);
        return float4((float3(.039,.057,.077)+gridLine*.025)*shadow*fade+float3(.026,.041,.067)*(1-fade),1);
    }
    float4 texel=Skin.Sample(SkinSampler,i.uv);
    if(i.part>=8) clip(texel.a-.1);
    float3 base=texel.rgb;
    float light=saturate(dot(n,normalize(float3(-.6,.85,-.65))));
    float rim=pow(1-saturate(abs(n.z)),3)*.11;
    return float4(base*(.35+light*.85)+float3(.44,.7,.75)*rim,1);
}
struct Particle { float4 position : SV_Position; float2 uv : TEXCOORD0; float3 color : TEXCOORD1; };
Particle ParticleVS(uint id : SV_VertexID, uint instance : SV_InstanceID) {
    Particle o;
    float2 corner=float2((id==1||id==2||id==4)?1:-1,(id==2||id==4||id==5)?1:-1);
    float t=clock.x*.42, k=(float)instance, u=k/768;
    float a=k*2.399963, r=sqrt(u)*.65;
    float3 disk=float3(cos(a)*r,sin(a)*r,0);
    float theta=u*PI*2*3;
    float3 knot=float3((sin(theta)+2*sin(2*theta))*.18,(cos(theta)-2*cos(2*theta))*.18,-sin(3*theta)*.28);
    // Damped response: position and overshoot are evaluated per instance.
    float phase=fmod(clock.x,14), age=fmod(phase,7);
    float spring=1-exp(-age*1.9)*(cos(age*5)+.38*sin(age*5));
    float blend=phase<7 ? spring : 1-spring;
    float3 p=lerp(disk,knot,blend);
    p.xz=rotate(p.xz,t); p.xy=rotate(p.xy,sin(t*.4)*.2);
    float depth=1/(1.7+p.z), radius=(.005+hash(k)*.005)*depth;
    float2 center=p.xy*depth*1.5;
    center.x/=clock.y/clock.z;
    o.position=float4(center+corner*radius*float2(clock.z/clock.y,1),.5,1);
    o.uv=corner;
    o.color=lerp(float3(.35,.65,.91),float3(.79,.94,.51),u)*(.65+depth*.6);
    return o;
}
float4 ParticlePS(Particle i) : SV_Target {
    float a=saturate((1-length(i.uv))*3);
    return float4(i.color*a,a);
}
