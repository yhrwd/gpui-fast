// Original analytic material study: brushed anodized stock, turned aluminium,
// knurling, engraved scales, recessed glass and a travelling spindle indicator.
// All distances are in the panel's 900 x 500 design coordinates.
cbuffer Frame : register(b0) {
    float4 clock;
    float4 camera; // encoder, feed, power, momentary press
};
struct Screen { float4 position : SV_Position; float2 uv : TEXCOORD0; };
static const float PI = 3.14159265359;
float noise(float2 p) { return frac(sin(dot(p,float2(12.9898,78.233)))*43758.5453); }
float box(float2 p, float2 h, float r) {
    float2 q=abs(p)-h+r; return length(max(q,0))+min(max(q.x,q.y),0)-r;
}
float coverage(float d) { return 1-smoothstep(-.65,.65,d); }
float circle(float2 p, float r) { return coverage(length(p)-r); }
float3 over(float3 bg, float3 fg, float alpha) { return lerp(bg,fg,saturate(alpha)); }
float3 screw(float3 c, float2 p) {
    float r=length(p);
    c=over(c,float3(.035,.038,.041),circle(p-float2(0,1),8));
    float light=.39+.18*(-p.y-p.x)/8+.05*cos(r*5);
    c=over(c,light.xxx,circle(p,6.6));
    c=over(c,float3(.07,.075,.08),coverage(box(p,float2(3.6,1.05),.7)));
    c=over(c,float3(.52,.53,.54),coverage(box(p-float2(0,1.2),float2(3.2,.22),.15)));
    return c;
}
float4 CncPS(Screen i) : SV_Target {
    float2 p=i.uv*float2(900,500);
    float2 centered=p-float2(450,250);
    float grain=(noise(float2(floor(p.x*.7),floor(p.y*3)))-.5)*.025;
    float brush=sin(p.y*5.8+sin(p.x*.005))*.009+sin(p.y*12.3)*.004;
    float d=box(centered,float2(447,247),20);
    float3 c=float3(.075,.08,.087);
    float metal=.235+grain+brush+.055*(1-p.y/500.)+.035*exp(-pow((p.x-260)/400,2));
    c=over(c,float3(metal*.96,metal*.99,metal*1.025),coverage(d));
    // Continuous top chamfer catches a broad softbox; bottom edge is occluded.
    float bevel=exp(-abs(d+3)*.7);
    c+=bevel*(p.y<250 ? .15 : -.09);
    float inset=box(centered,float2(428,228),12);
    c=over(c,float3(.075,.083,.09),exp(-abs(inset)*1.5)*.6);
    c=screw(c,p-float2(29,29)); c=screw(c,p-float2(871,29));
    c=screw(c,p-float2(29,471)); c=screw(c,p-float2(871,471));
    // Etched divider and aluminium identification badge.
    c=over(c,float3(.11,.12,.13),coverage(box(p-float2(450,78),float2(399,.6),.2)));
    c=over(c,float3(.38,.39,.40),coverage(box(p-float2(450,79.4),float2(399,.4),.2)));
    // Large rotary encoder. Dark socket, contact shadow, machined bezel.
    float2 q=p-float2(251,249);
    float r=length(q), angle=atan2(q.x,-q.y);
    c=over(c,float3(.07,.075,.08),circle(q-float2(0,9),106));
    float bezel=.34+.19*saturate((-q.y-q.x*.4)/110)+.055*cos(r*2.6);
    c=over(c,bezel.xxx,circle(q,103));
    c=over(c,float3(.08,.085,.09),circle(q,96));
    // 48 engraved ticks over a 270 degree range, with five longer divisions.
    float tickStep=4.72/48;
    float tickIndex=round((angle+2.36)/tickStep);
    float tickAngle=-2.36+tickIndex*tickStep;
    float major=1-step(.05,abs(fmod(tickIndex,4)));
    float tickDistance=abs(angle-tickAngle)*r;
    float tick=coverage(tickDistance-(major>.5 ? 1.0 : .55))*step(109,r)*step(r,major>.5 ? 124 : 117)*step(abs(angle),2.38);
    c=over(c,float3(.69,.71,.70),tick);
    float arc=coverage(abs(r-130)-1.25)*step(abs(angle),2.36);
    c=over(c,float3(.13,.15,.16),arc);
    float arcActive=arc*step(angle,-2.36+camera.x*4.72)*camera.z;
    c=over(c,float3(.98,.46,.16),arcActive);
    // Fluted sidewall with narrow specular edges; grooves follow the encoder.
    float rotation=-2.36+camera.x*4.72;
    float knurl=pow(saturate(.5+.5*cos((angle-rotation)*80)),5);
    float side=.30+.19*knurl+.15*saturate((-q.y-q.x)/110);
    c=over(c,side.xxx,circle(q,88));
    float face=.62+.19*saturate((-q.y-q.x*.4)/92);
    face+=.011*sin(r*2.3)*(1-smoothstep(1.1,2.8,fwidth(r)*2.3))+.025*cos((angle-rotation)*2)+grain;
    face-=.14*smoothstep(74,82,r);
    c=over(c,float3(face*.99,face,face*1.025),circle(q-float2(0,-2),80));
    c=over(c,float3(.94,.95,.96),exp(-abs(r-77)*1.5)*.5);
    // Turned centre cap and a recessed pointer with orange enamel.
    c=over(c,float3(.33,.35,.37),exp(-abs(r-47)*1.4)*.28);
    float2 direction=float2(sin(rotation),-cos(rotation));
    float2 tangent=float2(direction.y,-direction.x);
    float2 mark=float2(dot(q,direction)-63,dot(q,tangent));
    c=over(c,float3(.15,.17,.18),coverage(box(mark,float2(9,3.6),2)));
    c=over(c,float3(1,.45,.13),coverage(box(mark-float2(0,-.7),float2(7.8,2.1),1.5)));
    // Black glass display: gasket, top lip and subtle horizontal scan texture.
    float2 screen=p-float2(652,178);
    float glass=box(screen,float2(180,82),12);
    c=over(c,float3(.1,.11,.12),coverage(glass-4));
    c=over(c,float3(.39,.40,.41),coverage(glass-2));
    c=over(c,float3(.022,.036,.038)+float3(.025,.038,.032)*exp(-dot(screen/150,screen/150)),coverage(glass));
    c+=coverage(glass)*.006*sin(p.y*3);
    c+=coverage(glass)*float3(.04,.05,.05)*exp(-pow((screen.y+76)/3,2));
    // Sixteen LED segments show load, coupled to power and encoder position.
    float2 meter=p-float2(497,231);
    float segment=floor(meter.x/19);
    float led=coverage(box(float2(fmod(max(meter.x,0),19)-6,meter.y),float2(5.6,3),1));
    led*=step(0,meter.x)*step(meter.x,302);
    float lit=step(segment,3+camera.x*10)*camera.z;
    c=over(c,lerp(float3(.1,.16,.14),float3(.65,.93,.61),lit),led);
    // Feed rail and a short-throw knurled fader.
    float2 rail=p-float2(652,317);
    c=over(c,float3(.1,.11,.12),coverage(box(rail,float2(160,5),4)));
    c=over(c,float3(.49,.26,.13),coverage(box(rail-float2(-160+camera.y*160,0),float2(camera.y*160,1),1)));
    float2 fader=rail-float2(-160+camera.y*320,0);
    c=over(c,float3(.075,.08,.085),coverage(box(fader-float2(0,4),float2(15,20),4)));
    float faderMetal=.58-.12*fader.y/19+.05*cos(fader.x*2.8);
    c=over(c,faderMetal.xxx,coverage(box(fader,float2(14,18),4)));
    c=over(c,float3(.98,.47,.18),coverage(box(fader,float2(1.1,11),.6)));
    // Momentary metal button: bevel and shadow compress as it is depressed.
    float2 button=p-float2(730,407+camera.w*2.8);
    c=over(c,float3(.08,.09,.10),coverage(box(button-float2(0,4-camera.w*3),float2(101,31),7)));
    float bd=box(button,float2(99,28),6);
    float cap=.43-.09*button.y/28-camera.w*.12;
    c=over(c,float3(cap*1.06,cap,cap*.94),coverage(bd));
    c+=exp(-abs(bd+1.7)*1.4)*.09*(1-camera.w);
    // Recessed power rocker; thumb travels as the spindle is enabled.
    float2 rocker=p-float2(529,407);
    c=over(c,float3(.07,.08,.085),coverage(box(rocker,float2(52,29),25)));
    c=over(c,float3(.16,.18,.19),coverage(box(rocker,float2(48,25),23)));
    float2 thumb=rocker-float2(-23+46*camera.z,-1);
    float thumbMetal=.51-.16*thumb.y/21+.018*sin(thumb.y*4);
    c=over(c,thumbMetal.xxx,circle(thumb,21));
    c=over(c,lerp(float3(.2,.23,.22),float3(.63,.94,.63),camera.z),circle(p-float2(574,369),3.5));
    // Tiny orbit on glass communicates spindle velocity rather than idle motion.
    float2 rotor=p-float2(791,142);
    float rotorAngle=atan2(rotor.y,rotor.x)-clock.x*(1+camera.x*5);
    float rotorInk=coverage(abs(length(rotor)-13)-1.4)*(.24+.76*pow(.5+.5*cos(rotorAngle),5));
    c=over(c,float3(.63,.91,.65),rotorInk*camera.z);
    return float4(saturate(c),1);
}
