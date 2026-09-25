<!-- Text and a following unit as the content of an inline element that trailing text follows after a space, at the 100/101 boundary of the content line: at 100 the text and the unit share the line, at 101 the unit starts a fresh line and the text keeps its own, the same form the element takes without the trailing text. -->

<!-- content line == 100: the text and an {#await} share the line -->
<div>lead <span>text {#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}B{/await}</span> tail</div>

<!-- content line == 101: the {#await} starts a fresh line -->
<div>lead <span>text {#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}B{/await}</span> tail</div>

<!-- content line == 100: the text and an inline element share the line -->
<div>lead <span>text <i>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</i></span> tail</div>

<!-- content line == 101: the inline element starts a fresh line -->
<div>lead <span>text <i>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx</i></span> tail</div>

<!-- content line == 100: the text and an expression tag share the line -->
<div>lead <span>text {aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}</span> tail</div>

<!-- content line == 101: the expression tag starts a fresh line -->
<div>lead <span>text {aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}</span> tail</div>

<!-- content line == 100 with no text before the element: the text and an {#await} share the line -->
<div><span>text {#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}B{/await}</span> tail</div>

<!-- content line == 101 with no text before the element: the {#await} starts a fresh line -->
<div><span>text {#await aaaa || xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx}B{/await}</span> tail</div>
