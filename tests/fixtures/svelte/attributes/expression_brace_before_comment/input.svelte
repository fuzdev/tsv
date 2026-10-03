<!-- a `}` inside a template literal, a regex, or a string ends no expression: the
	comment after it is inside the expression, not between the attributes -->
<div data-attr={() => `}` + /* c1 */ a} data-attr1="value"></div>
<div data-attr={fn(/}/) + /* c2 */ a} data-attr1="value"></div>
<div data-attr={fn('}') + /* c3 */ a} data-attr1="value"></div>

<!-- a comment between the attributes, after an expression holding a `}` -->
<div data-attr={`}`} /* c4 */ data-attr1="value"></div>
