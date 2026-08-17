<!-- A same-line block comment trails the token before it, exactly as a same-line `//` does.
	The two only look alike while the list fits on one line; once it wraps, each gap shows
	whether the comment stayed with the token it was written after. -->

<!-- At exactly 100 the list stays inline and both formatters agree -->
<div/* c */data-attr1="value1" data-attr2="value2" data-attr3="value3" data-attr4="value4a"></div>

<!-- At 101 it wraps and the comment stays trailing the tag name -->
<!-- (prettier moves it down onto the first attribute's line) -->
<div/* c */data-attr1="value1" data-attr2="value2" data-attr3="value3" data-attr4="value4ab"></div>

<!-- Between attributes it trails the one before it -->
<!-- (prettier moves it down onto the next one) -->
<div data-attr1="value1"/* c */data-attr2="value2" data-attr3="value3" data-attr4="value4" data-attr5="value5"></div>

<!-- After the last attribute both formatters already trail it -->
<div data-attr1="value1" data-attr2="value2" data-attr3="value3" data-attr4="value4" data-attr5="value5"/* c */></div>
