<div><script>console.log("hello");const x=1;</script></div>
<div><script></script></div>

<!-- with attributes: the `>` hugs the last attribute when they fit -->
<div><script    defer     data-attr="value">const a=1;</script></div>

<div><script     data-attr="value"></script></div>

<!-- blank lines between statements and around a comment are kept -->
<div><script>const b=1;

// c

const d=2;</script></div>
