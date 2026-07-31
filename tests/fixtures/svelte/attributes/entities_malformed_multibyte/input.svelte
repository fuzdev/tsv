<!--
	A malformed named reference whose name run carries a NON-ASCII character. Named
	character references are ASCII-only, so no such run can ever name an entity: the
	scan stops at the first non-ASCII character and the source is preserved verbatim.
	Cases: a multibyte character inside a semicolon-terminated run; the same with no
	semicolon; and a well-formed entity beside it (the control that still decodes).
-->
<div data-attr="a&b中pos;s"></div>
<div data-attr="a&b中pos"></div>
<div data-attr="&amp; a&b中pos;"></div>
