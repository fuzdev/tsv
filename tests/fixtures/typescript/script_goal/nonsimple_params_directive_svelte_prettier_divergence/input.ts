// the parameters parse under the OUTER (sloppy) mode: only the body is turned strict by
// its own directive, so a legacy octal default is legal here even though the same
// literal in the body would not be
function fn(a = 010) {
	'use strict';
	return a;
}

// the arrow and method spellings take the same reading
const arrow = (a = 010) => {
	'use strict';
	return a;
};

const obj = {
	m(a = 010) {
		'use strict';
		return a;
	}
};
