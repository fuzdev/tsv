<!-- adjacent expressions read as one value, not separate attributes -->
<div data-attr1="{a}{b}"></div>

<!-- expression followed by text -->
<div data-attr2="{a}//text1"></div>

<!-- text followed by expression -->
<div data-attr3="text1{a}"></div>

<!-- a bare slash is value content; only `/>` terminates an unquoted value -->
<a href="/text1/text2">link</a>

<!-- `/>` self-close terminates the value before the slash -->
<input data-attr4="value" />
