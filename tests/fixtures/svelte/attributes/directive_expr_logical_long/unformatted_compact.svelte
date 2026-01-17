<!-- class: directive with logical expressions - boundary tests -->

<!-- 100 chars - stays inline (at boundary) -->
<input class:active={condA && condB && condC && condD && condE && condF && condG && condH && condIJKLMNO} />

<!-- 101 chars - breaks at operators -->
<input class:active={condA && condB && condC && condD && condE && condF && condG && condH && condIJKLMNOP} />

<!-- Deeply nested (6 tabs) - breaks at operators due to reduced available width -->
<div><div><div><div><div><button class:active={aaaaaa.connected && aaaaaa.url !== aaaaaa.url_input && aaaaaa.url_input !== ''}>text</button></div></div></div></div></div>
