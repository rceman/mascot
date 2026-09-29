# capture-shadcn.ps1 — dev-only shadcn/ui reference capture.
#
# Drives headless Chrome (Edge fallback) over the Chrome DevTools Protocol
# via System.Net.WebSockets.ClientWebSocket. PowerShell 5.1 compatible; no
# Node/Python required. Captures component screenshots from the official
# https://ui.shadcn.com/view/new-york-v4/<example> pages at DPR 2, light and
# dark, with per-file computed styles + provenance.
#
# Output: <OutDir>/*.png, provenance.json, PROVENANCE.md.
# Fails loudly: any capture error aborts and the output dir is removed (it is
# staged in <OutDir>.tmp and moved into place only on full success).
#
# Dev tooling only — nothing in the Rust workspace may consume these files
# except the lab component gallery.

[CmdletBinding()]
param(
    [string]$OutDir = (Join-Path $PSScriptRoot 'shadcn'),
    [int]$Port = 9333
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$ViewBase = 'https://ui.shadcn.com/view/new-york-v4'
$DocsBase = 'https://ui.shadcn.com/docs/components'
$Pad = 16         # CSS px of context around the element box (rework r2)
$Dsf = 2          # device scale factor
$VpDefault = 1200 # CSS px viewport width — every capture stays >= 800 CSS px
                  # so the page never drops under Tailwind's `md` breakpoint
                  # (a 430-px viewport made textarea render text-base instead
                  # of md:text-sm). To land a ~380 px wide element we set an
                  # inline width on the element instead of shrinking the page.

# ---------------------------------------------------------------- targets
# sel: evaluated in the page; must return the element to clip.
# styleSel: element the computedStyle + rect are read from (omitted = sel;
#           for the tooltip the content, not the trigger).
# w / mw: inline style width / max-width (CSS px) applied after load so the
#         element renders at a native-comparable width without shrinking the
#         viewport below the `md` breakpoint.
# state: default | hover | focus-visible | disabled | open (tooltip)
# variant/size: expected data-* attribute values — asserted before capture.
$Targets = @(
    # Button — default variant from the dedicated single-variant page
    @{ file = 'button-default';             example = 'button-default';   component = 'Button';     sel = '[data-slot="button"]'; state = 'default';       variant = 'default';   size = 'default'; docs = 'button' }
    @{ file = 'button-hover';               example = 'button-default';   component = 'Button';     sel = '[data-slot="button"]'; state = 'hover';         variant = 'default';   size = 'default'; docs = 'button' }
    @{ file = 'button-focus-visible';       example = 'button-default';   component = 'Button';     sel = '[data-slot="button"]'; state = 'focus-visible'; variant = 'default';   size = 'default'; docs = 'button' }
    @{ file = 'button-disabled';            example = 'button-default';   component = 'Button';     sel = '[data-slot="button"]'; state = 'disabled';      variant = 'default';   size = 'default'; docs = 'button' }
    @{ file = 'button-secondary';           example = 'button-secondary'; component = 'Button';     sel = '[data-slot="button"]'; state = 'default';       variant = 'secondary'; docs = 'button' }
    @{ file = 'button-secondary-hover';     example = 'button-secondary'; component = 'Button';     sel = '[data-slot="button"]'; state = 'hover';         variant = 'secondary'; docs = 'button' }
    @{ file = 'button-ghost';               example = 'button-ghost';     component = 'Button';     sel = '[data-slot="button"]'; state = 'default';       variant = 'ghost';     docs = 'button' }
    @{ file = 'button-ghost-hover';         example = 'button-ghost';     component = 'Button';     sel = '[data-slot="button"]'; state = 'hover';         variant = 'ghost';     docs = 'button' }
    # IconButton — shadcn has no primary icon button; the icon demo is
    # outline/size-icon, recorded as such in provenance
    @{ file = 'button-icon';                example = 'button-icon';      component = 'IconButton'; sel = '[data-slot="button"]'; state = 'default';       variant = 'outline';   size = 'icon';    docs = 'button' }
    @{ file = 'button-icon-hover';          example = 'button-icon';      component = 'IconButton'; sel = '[data-slot="button"]'; state = 'hover';         variant = 'outline';   size = 'icon';    docs = 'button' }
    @{ file = 'button-icon-focus-visible';  example = 'button-icon';      component = 'IconButton'; sel = '[data-slot="button"]'; state = 'focus-visible'; variant = 'outline';   size = 'icon';    docs = 'button' }
    # Badge
    @{ file = 'badge-default';              example = 'badge-demo';       component = 'Badge';      sel = '[data-slot="badge"]';  state = 'default';       variant = 'default';   docs = 'badge' }
    @{ file = 'badge-secondary';            example = 'badge-secondary';  component = 'Badge';      sel = '[data-slot="badge"]';  state = 'default';       variant = 'secondary'; docs = 'badge' }
    @{ file = 'badge-outline';              example = 'badge-outline';    component = 'Badge';      sel = '[data-slot="badge"]';  state = 'default';       variant = 'outline';   docs = 'badge' }
    # Tooltip (open state; capture = union(trigger, content); style/rect read
    # from the tooltip CONTENT, not the trigger)
    @{ file = 'tooltip-open';               example = 'tooltip-demo';     component = 'Tooltip';    sel = 'button';               styleSel = '[data-slot="tooltip-content"]'; state = 'open'; docs = 'tooltip' }
    # Separator
    @{ file = 'separator';                  example = 'separator-demo';   component = 'Separator';  sel = '[data-slot="separator"]'; state = 'default';    w = '380px'; docs = 'separator' }
    # Composer analogue = textarea (~380 CSS px via inline width, not a narrow
    # viewport — see $VpDefault note)
    @{ file = 'textarea-default';           example = 'textarea-demo';    component = 'Composer';   sel = '[data-slot="textarea"]'; state = 'default';      w = '380px'; docs = 'textarea' }
    @{ file = 'textarea-focus-visible';     example = 'textarea-demo';    component = 'Composer';   sel = '[data-slot="textarea"]'; state = 'focus-visible'; w = '380px'; docs = 'textarea' }
    @{ file = 'textarea-disabled';          example = 'textarea-disabled';component = 'Composer';   sel = '[data-slot="textarea"]'; state = 'disabled';     w = '380px'; docs = 'textarea' }
    # Surface analogue = card (natural width — never constrained)
    @{ file = 'card';                       example = 'card-demo';        component = 'Surface';    sel = '[data-slot="card"]';   state = 'default';       docs = 'card' }
    # Typography (max-width:380px so line breaks compare with native cells)
    @{ file = 'typography-p';               example = 'typography-p';     component = 'Typography'; sel = 'p:first-of-type';      state = 'default';       mw = '380px'; docs = 'typography' }
    @{ file = 'typography-muted';           example = 'typography-muted'; component = 'Typography'; sel = 'p:first-of-type';      state = 'default';       mw = '380px'; docs = 'typography' }
    @{ file = 'typography-small';           example = 'typography-small'; component = 'Typography'; sel = 'small:first-of-type';  state = 'default';       mw = '380px'; docs = 'typography' }
)

# ---------------------------------------------------------------- helpers
$script:ws = $null
$script:nextId = 0

function Prop($o, [string]$n) {
    if ($null -eq $o) { return $null }
    if ($o -is [System.Collections.IDictionary]) {
        if ($o.Contains($n)) { return $o[$n] }
        return $null
    }
    if ($null -ne $o.PSObject.Properties[$n]) { return $o.$n }
    return $null
}

function Send-Cdp([string]$Method, $Params) {
    $script:nextId++
    $id = $script:nextId
    $msg = @{ id = $id; method = $Method }
    if ($null -ne $Params) { $msg.params = $Params }
    $bytes = [Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -Compress -Depth 10 $msg))
    $seg = New-Object ArraySegment[byte] -ArgumentList (, $bytes)
    $script:ws.SendAsync($seg, [Net.WebSockets.WebSocketMessageType]::Text, $true,
        [Threading.CancellationToken]::None).Wait()
    # read until the response for our id arrives (events are skipped); CDP
    # replies can span many socket frames — accumulate bytes, not strings
    $buf = New-Object byte[] (256KB)
    while ($true) {
        $ms = New-Object IO.MemoryStream
        while ($true) {
            $seg = New-Object ArraySegment[byte] -ArgumentList (, $buf)
            $task = $script:ws.ReceiveAsync($seg, [Threading.CancellationToken]::None)
            $task.Wait()
            $ms.Write($buf, 0, $task.Result.Count)
            if ($task.Result.EndOfMessage) { break }
        }
        $text = [Text.Encoding]::UTF8.GetString($ms.ToArray())
        $ms.Dispose()
        $obj = $null
        try { $obj = $text | ConvertFrom-Json } catch { continue }
        if ($null -eq (Prop $obj 'id')) { continue }              # event
        if ($obj.id -ne $id) { continue }                         # stale
        $err = Prop $obj 'error'
        if ($err) { throw "CDP $Method failed: $(Prop $err 'message')" }
        return (Prop $obj 'result')
    }
}

function Eval([string]$Expr, [switch]$AwaitPromise) {
    $p = @{ expression = $Expr; returnByValue = $true }
    if ($AwaitPromise) { $p.awaitPromise = $true }
    $r = Send-Cdp 'Runtime.evaluate' $p
    $ex = Prop $r 'exceptionDetails'
    if ($ex) { throw "eval failed: $Expr -> $(Prop $ex 'text')" }
    $res = Prop $r 'result'
    return (Prop $res 'value')
}

function Wait-For([string]$Expr, [int]$TimeoutMs = 10000, [string]$What = 'condition') {
    $sw = [Diagnostics.Stopwatch]::StartNew()
    while ($sw.ElapsedMilliseconds -lt $TimeoutMs) {
        if (Eval $Expr -eq $true) { return }
        Start-Sleep -Milliseconds 120
    }
    throw "timeout waiting for $What"
}

function Element-Rect([string]$Sel) {
    $js = "(function(){var el=document.querySelector('$Sel');if(!el)return null;var r=el.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height};})()"
    $r = Eval $js
    if ($null -eq $r) { throw "element not found: $Sel" }
    return $r
}

function Set-Viewport([int]$Width) {
    Send-Cdp 'Emulation.setDeviceMetricsOverride' @{
        width = $Width; height = 900; deviceScaleFactor = $Dsf; mobile = $false
    } | Out-Null
}

# Wait until the page has visually settled: two animation frames + the
# transition-all budget shadcn uses (~250 ms).
function Wait-Settle {
    Eval 'new Promise(function(r){requestAnimationFrame(function(){requestAnimationFrame(function(){r(true)})})})' -AwaitPromise | Out-Null
    Start-Sleep -Milliseconds 280
}

# Assert the page background actually matches the requested theme — guards
# against reading computed styles mid-theme-flip.
function Assert-Theme([string]$Theme) {
    # any CSS colour syntax (rgb, lab, oklch, ...) is normalised to sRGB via
    # a 1px canvas readback
    $js = @'
(function(){
function norm(s){var c=document.createElement('canvas');c.width=c.height=1;
var g=c.getContext('2d');g.fillStyle=s;g.fillRect(0,0,1,1);
return g.getImageData(0,0,1,1).data;}
function probe(el){var s=getComputedStyle(el).backgroundColor;var d=norm(s);
if(d[3]<76)return null;var lum=(0.2126*d[0]+0.7152*d[1]+0.0722*d[2])/255;
return lum>0.5?'light':'dark';}
var el=document.body;
while(el){var t=probe(el);if(t)return t;el=el.parentElement;}
return 'transparent';
})()
'@
    $got = Eval $js
    if ($got -ne $Theme) { throw "page background reads '$got', expected theme '$Theme' — styles would be recorded for the wrong theme" }
}

# ---------------------------------------------------------------- browser
$chrome = 'C:\Program Files\Google\Chrome\Application\chrome.exe'
$edge = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
$browser = if (Test-Path $chrome) { $chrome } elseif (Test-Path $edge) { $edge } else { throw 'no Chrome or Edge found' }
$browserName = Split-Path $browser -Leaf
$browserVersion = (Get-Item $browser).VersionInfo.ProductVersion
"using $browserName $browserVersion on port $Port"

$profileDir = Join-Path $env:TEMP ("shadcn-cdp-" + [guid]::NewGuid().ToString('N'))
$tmpOut = "$OutDir.tmp"
if (Test-Path $tmpOut) { Remove-Item -Recurse -Force $tmpOut }
New-Item -ItemType Directory -Force $tmpOut | Out-Null

$proc = Start-Process -PassThru -FilePath $browser -ArgumentList @(
    '--headless=new',
    "--remote-debugging-port=$Port",
    "--user-data-dir=$profileDir",
    '--no-first-run', '--no-default-browser-check',
    '--window-size=1200,900',
    'about:blank')

try {
    # wait for the CDP HTTP endpoint, then connect the page websocket
    $target = $null
    foreach ($i in 0..80) {
        try {
            $list = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json" -TimeoutSec 2
            $target = $list | Where-Object { $_.type -eq 'page' } | Select-Object -First 1
            if ($target) { break }
        } catch { }
        Start-Sleep -Milliseconds 250
    }
    if (-not $target) { throw 'CDP endpoint never came up' }

    $script:ws = New-Object Net.WebSockets.ClientWebSocket
    $script:ws.ConnectAsync([Uri]$target.webSocketDebuggerUrl,
        [Threading.CancellationToken]::None).Wait()

    Send-Cdp 'Page.enable' $null | Out-Null
    Send-Cdp 'Runtime.enable' $null | Out-Null
    Set-Viewport $VpDefault

    # upstream commit sha (best effort; ui.shadcn.com exposes no build id —
    # /_next/static/immutable/ is a cache path, not a build id)
    $commitSha = 'unavailable'
    try {
        $c = Invoke-RestMethod -Uri 'https://api.github.com/repos/shadcn-ui/ui/commits/main' -TimeoutSec 10
        $commitSha = $c.sha
    } catch { }

    $provenance = New-Object System.Collections.ArrayList
    $errors = New-Object System.Collections.ArrayList
    $themeVars = @{}

    # hex() self-test: translucent colours must keep their RGB bits (the
    # premultiplied canvas round-trip on the raw colour loses them — RGB is
    # read from the opaque variant, alpha from the original draw)
    $hexCheck = Eval @'
(function(){
function hex(v){var c=document.createElement('canvas');c.width=c.height=1;
var g=c.getContext('2d');
g.fillStyle=v;g.fillRect(0,0,1,1);
var a=g.getImageData(0,0,1,1).data[3];
g.clearRect(0,0,1,1);
g.fillStyle='rgb(from '+v+' r g b / 1)';g.fillRect(0,0,1,1);
var d=g.getImageData(0,0,1,1).data;
function h(n){return ('0'+d[n].toString(16)).slice(-2);}
var o='#'+h(0)+h(1)+h(2);if(a<255)o+=('0'+a.toString(16)).slice(-2);return o.toUpperCase();}
return [hex('lab(100% 0 0 / .1)'), hex('oklch(1 0 0 / 15%)')];})()
'@
    if ("$($hexCheck[0])" -ne '#FFFFFF1A' -or "$($hexCheck[1])" -ne '#FFFFFF26') {
        throw "hex() self-test failed: lab(100% 0 0 /.1) -> $($hexCheck[0]), oklch(1 0 0 /15%) -> $($hexCheck[1])"
    }
    Write-Output "hex() self-test: $($hexCheck -join ', ')"

    foreach ($t in $Targets) {
        $vp = $VpDefault
        foreach ($theme in 'light', 'dark') {
            $file = "$($t.file)-$theme.png"
            $url = "$ViewBase/$($t.example)"
            try {
                Set-Viewport $vp
                # pin prefers-color-scheme before navigation so nothing
                # (page JS, media queries) can re-apply the other theme
                Send-Cdp 'Emulation.setEmulatedMedia' @{
                    features = @(@{ name = 'prefers-color-scheme'; value = $theme })
                } | Out-Null
                Send-Cdp 'Page.navigate' @{ url = $url } | Out-Null
                Wait-For 'document.readyState === "complete"' 10000 'page load'
                # theme: toggle the `dark` class + colorScheme, then paint the
                # theme's --background on html/body (the view pages leave
                # them transparent; screenshots and the theme assert below
                # need the real page colour)
                if ($theme -eq 'dark') {
                    Eval 'document.documentElement.classList.add("dark");document.documentElement.style.colorScheme="dark";true' | Out-Null
                } else {
                    Eval 'document.documentElement.classList.remove("dark");document.documentElement.style.colorScheme="light";true' | Out-Null
                }
                Eval 'document.documentElement.style.backgroundColor="var(--background)";document.body.style.backgroundColor="var(--background)";true' | Out-Null
                Eval 'document.fonts.ready.then(()=>true)' -AwaitPromise | Out-Null
                Wait-For "!!document.querySelector('$($t.sel)')" 10000 "$($t.sel)"

                # inline width / max-width so the element renders at a
                # native-comparable width while the viewport stays >= 800
                $styleHow = ''
                if ($null -ne (Prop $t 'w')) {
                    Eval "(function(){document.querySelector('$($t.sel)').style.width='$($t.w)';return true;})()" | Out-Null
                    $styleHow = "; element.style.width='$($t.w)'"
                }
                if ($null -ne (Prop $t 'mw')) {
                    Eval "(function(){document.querySelector('$($t.sel)').style.maxWidth='$($t.mw)';return true;})()" | Out-Null
                    $styleHow = "; element.style.maxWidth='$($t.mw)'"
                }
                Wait-Settle

                $how = 'rendered state' + $styleHow
                switch ($t.state) {
                    'hover' {
                        $r = Element-Rect $t.sel
                        Send-Cdp 'Input.dispatchMouseEvent' @{
                            type = 'mouseMoved'
                            x    = [double]($r.x + $r.width / 2)
                            y    = [double]($r.y + $r.height / 2)
                        } | Out-Null
                        $how = 'CDP Input.dispatchMouseEvent mouseMoved to element centre'
                    }
                    'focus-visible' {
                        Eval "(function(){var el=document.querySelector('$($t.sel)');el.focus({focusVisible:true});return true;})()" | Out-Null
                        $how = 'el.focus({focusVisible:true}) via CDP Runtime.evaluate'
                    }
                    'disabled' {
                        Eval "(function(){var el=document.querySelector('$($t.sel)');if(!el.hasAttribute('disabled'))el.setAttribute('disabled','');el.setAttribute('aria-disabled','true');return true;})()" | Out-Null
                        $how = 'disabled attribute set via CDP (page element was enabled)'
                    }
                    'open' {
                        $r = Element-Rect $t.sel
                        Send-Cdp 'Input.dispatchMouseEvent' @{
                            type = 'mouseMoved'
                            x    = [double]($r.x + $r.width / 2)
                            y    = [double]($r.y + $r.height / 2)
                        } | Out-Null
                        Wait-For '!!document.querySelector("[data-slot=\"tooltip-content\"]")' 5000 'tooltip content'
                        $how = 'trigger hovered via CDP mouseMoved; tooltip-content awaited'
                    }
                }
                Wait-Settle
                Assert-Theme $theme

                # root theme custom properties, once per theme (raw + sRGB hex)
                if (-not $themeVars.ContainsKey($theme)) {
                    $themeVars[$theme] = Eval @'
(function(){
var el=document.documentElement;var s=getComputedStyle(el);
var names=['--radius','--background','--foreground','--card','--card-foreground',
'--popover','--primary','--primary-foreground','--secondary','--secondary-foreground',
'--muted','--muted-foreground','--accent','--accent-foreground','--border','--input','--ring'];
function hex(v){var c=document.createElement('canvas');c.width=c.height=1;
var g=c.getContext('2d');
g.fillStyle=v;g.fillRect(0,0,1,1);
var a=g.getImageData(0,0,1,1).data[3];
g.clearRect(0,0,1,1);
g.fillStyle='rgb(from '+v+' r g b / 1)';g.fillRect(0,0,1,1);
var d=g.getImageData(0,0,1,1).data;
function h(n){return ('0'+d[n].toString(16)).slice(-2);}
var out='#'+h(0)+h(1)+h(2);if(a<255)out+=('0'+a.toString(16)).slice(-2);return out.toUpperCase();}
var out={};for(var i=0;i<names.length;i++){var v=s.getPropertyValue(names[i]).trim();
out[names[i]]={raw:v,hex:v?hex(v):null};}return out;})()
'@
                }

                # assert the captured element is the variant/size we claim
                $attrs = Eval "(function(){var el=document.querySelector('$($t.sel)');if(!el)return null;return {variant:el.dataset.variant||'',size:el.dataset.size||'',text:(el.innerText||'').trim().slice(0,300)};})()"
                if ($null -eq $attrs) { throw "element not found for attr check: $($t.sel)" }
                $gotV = "$($attrs.variant)"; $gotS = "$($attrs.size)"
                if ($null -ne (Prop $t 'variant') -and $gotV -ne $t.variant) {
                    throw "$($t.file): data-variant='$gotV', expected '$($t.variant)'"
                }
                if ($null -ne (Prop $t 'size') -and $gotS -ne $t.size) {
                    throw "$($t.file): data-size='$gotS', expected '$($t.size)'"
                }

                # isolate the target: body padding keeps it >= 32px off the
                # document edge so the +-16px clip never clips top/left;
                # every other element with the same data-slot plus any sibling
                # subtree inside the padded clip is hidden (visibility keeps
                # layout stable)
                $hidJs = @"
(function(){
if(!document.getElementById('__cap_pad')){var st=document.createElement('style');st.id='__cap_pad';st.textContent='body{padding:32px !important}';document.head.appendChild(st);}
var el=document.querySelector('$($t.sel)');if(!el)return -1;
var r=el.getBoundingClientRect();var pad=$Pad;var hidden=0;
var slot=el.getAttribute('data-slot');
if(slot){var same=document.querySelectorAll('[data-slot="'+slot+'"]');
for(var i=0;i<same.length;i++){if(same[i]!==el){same[i].style.visibility='hidden';hidden++;}}}
function hideSibs(e){var kids=e.parentElement?e.parentElement.children:[];for(var i=0;i<kids.length;i++){var s=kids[i];if(s===e)continue;var sr=s.getBoundingClientRect();if(!(sr.right<r.x-pad||sr.left>r.right+pad||sr.bottom<r.y-pad||sr.top>r.bottom+pad)){s.style.visibility='hidden';hidden++;}}}
hideSibs(el);
return hidden;})()
"@
                $hidden = Eval $hidJs
                $how += "; body padded 32px; same-slot + sibling intruders hidden x$hidden"

                # clip = element box (+ tooltip content) grown by $Pad CSS px
                # on ALL four sides; the margin self-check runs in the same
                # eval so a clip that can't supply 16 CSS px fails the capture
                $clipJs = @"
(function(){
var el=document.querySelector('$($t.sel)');if(!el)return null;
var r=el.getBoundingClientRect();
var x0=r.x,y0=r.y,x1=r.right,y1=r.bottom;
var tip=document.querySelector('[data-slot="tooltip-content"]');
if(tip){var tr=tip.getBoundingClientRect();x0=Math.min(x0,tr.x);y0=Math.min(y0,tr.y);x1=Math.max(x1,tr.right);y1=Math.max(y1,tr.bottom);}
x0=Math.max(0,x0-$Pad);y0=Math.max(0,y0-$Pad);x1=x1+$Pad;y1=y1+$Pad;
var ok = (r.x-x0>=$Pad-0.01)&&(r.y-y0>=$Pad-0.01)&&(x1-r.right>=$Pad-0.01)&&(y1-r.bottom>=$Pad-0.01);
return {x:x0,y:y0,width:x1-x0,height:y1-y0,marginOk:ok};
})()
"@
                $clip = Eval $clipJs
                if ($null -eq $clip) { throw "clip eval failed for $($t.sel)" }
                if (-not $clip.marginOk) { throw "$($t.file): crop margin < $Pad CSS px on some side" }

                # computed style + border-box rect AFTER theme + state settle —
                # read from the style element (tooltip: the content, not the
                # trigger); every colour field gets an sRGB hex via canvas
                # normalisation (alpha kept as #RRGGBBAA)
                $styleSel = if ($null -ne (Prop $t 'styleSel')) { $t.styleSel } else { $t.sel }
                $meas = Eval @"
(function(){
function hex(v){var c=document.createElement('canvas');c.width=c.height=1;
var g=c.getContext('2d');
g.fillStyle=v;g.fillRect(0,0,1,1);
var a=g.getImageData(0,0,1,1).data[3];
g.clearRect(0,0,1,1);
g.fillStyle='rgb(from '+v+' r g b / 1)';g.fillRect(0,0,1,1);
var d=g.getImageData(0,0,1,1).data;
function h(n){return ('0'+d[n].toString(16)).slice(-2);}
var o='#'+h(0)+h(1)+h(2);if(a<255)o+=('0'+a.toString(16)).slice(-2);return o.toUpperCase();}
var el=document.querySelector('$styleSel');if(!el)return null;var s=getComputedStyle(el);
var r=el.getBoundingClientRect();
var fields={height:s.height,width:s.width,padding:s.padding,borderRadius:s.borderRadius,
borderWidth:s.borderWidth,borderColor:s.borderColor,backgroundColor:s.backgroundColor,
color:s.color,fontFamily:s.fontFamily,fontSize:s.fontSize,fontWeight:s.fontWeight,
lineHeight:s.lineHeight,boxShadow:s.boxShadow};
fields.borderColorHex=hex(s.borderColor);fields.backgroundColorHex=hex(s.backgroundColor);
fields.colorHex=hex(s.color);
return {rect:{width:r.width,height:r.height},style:fields};})()
"@
                if ($null -eq $meas) { throw "style element not found: $styleSel" }
                $styles = $meas.style
                $rect = $meas.rect

                $shot = Send-Cdp 'Page.captureScreenshot' @{
                    format = 'png'
                    clip   = @{ x = $clip.x; y = $clip.y; width = $clip.width; height = $clip.height; scale = 1 }
                }
                [IO.File]::WriteAllBytes((Join-Path $tmpOut $file), [Convert]::FromBase64String($shot.data))

                [void]$provenance.Add(@{
                        file          = $file
                        component     = $t.component
                        example       = $t.example
                        url           = $url
                        docsUrl       = "$DocsBase/$($t.docs)"
                        theme         = $theme
                        state         = $t.state
                        stateProduced = $how
                        dataVariant   = $gotV
                        dataSize      = $gotS
                        elementText   = "$($attrs.text)"
                        capturedUtc   = (Get-Date).ToUniversalTime().ToString('o')
                        browser       = "$browserName $browserVersion"
                        dpr           = $Dsf
                        viewportCss   = $vp
                        buildId       = 'unavailable'
                        commitSha     = $commitSha
                        rect          = $rect
                        styleElement  = $styleSel
                        computedStyle = $styles
                    })
                Write-Output "  captured $file"
            } catch {
                [void]$errors.Add("$file : $($_.Exception.Message)")
            }
        }
    }

    if ($errors.Count -gt 0) {
        throw "capture failures:`n" + ($errors -join "`n")
    }

    # provenance.json + PROVENANCE.md (UTF-8 without BOM — serde/strict parsers)
    # root = { themeVars: {light,dark}, captures: [...] }
    $utf8nb = New-Object Text.UTF8Encoding($false)
    $root = @{
        themeVars = @{
            light = $themeVars['light']
            dark  = $themeVars['dark']
        }
        captures  = $provenance
    }
    [IO.File]::WriteAllText((Join-Path $tmpOut 'provenance.json'),
        ($root | ConvertTo-Json -Depth 8), $utf8nb)

    $md = New-Object Text.StringBuilder
    [void]$md.AppendLine('# shadcn/ui reference captures — provenance')
    [void]$md.AppendLine('')
    [void]$md.AppendLine("Captured: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-dd HH:mm')) UTC")
    [void]$md.AppendLine("Browser: $browserName $browserVersion (headless, CDP, DPR $Dsf)")
    [void]$md.AppendLine("ui.shadcn.com Next.js build id: unavailable (/_next/static/immutable/ is a cache path); shadcn-ui/ui main commit: ``$commitSha``")
    [void]$md.AppendLine('')
    [void]$md.AppendLine('License: shadcn/ui is MIT-licensed; these PNGs are developer-only')
    [void]$md.AppendLine('reference evidence — never shipped or read at runtime.')
    [void]$md.AppendLine('')
    [void]$md.AppendLine('| file | component | example | variant/size | state | theme | used by gallery |')
    [void]$md.AppendLine('|---|---|---|---|---|---|---|')
    foreach ($p in $provenance) {
        $vs = "$(Prop $p 'dataVariant')/$(Prop $p 'dataSize')"
        [void]$md.AppendLine("| $($p.file) | $($p.component) | $($p.example) | $vs | $($p.state) | $($p.theme) | component-gallery-shadcn-reference*.png |")
    }
    [IO.File]::WriteAllText((Join-Path $tmpOut 'PROVENANCE.md'), $md.ToString(), $utf8nb)

    # publish atomically
    if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
    Move-Item $tmpOut $OutDir
    Write-Output "captured $($provenance.Count) files -> $OutDir"
} finally {
    if ($script:ws -and $script:ws.State -eq 'Open') {
        try { $script:ws.CloseAsync([Net.WebSockets.WebSocketCloseStatus]::NormalClosure, 'done', [Threading.CancellationToken]::None).Wait(2000) } catch { }
        $script:ws.Dispose()
    }
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
    Remove-Item -Recurse -Force $profileDir -ErrorAction SilentlyContinue
    if ((Test-Path $tmpOut) -and (Test-Path $OutDir)) { Remove-Item -Recurse -Force $tmpOut -ErrorAction SilentlyContinue }
}
