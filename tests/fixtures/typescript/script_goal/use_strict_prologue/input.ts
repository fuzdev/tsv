'use strict';

const a = 1;

function fn() {
	'use strict';
	return a;
}

const obj = {
	m() {
		'use strict';
		return a;
	}
};

const arrow = () => {
	'use strict';
	return a;
};

class Single {
	m() {
		'use strict';
		return a;
	}
}

// past the prologue, and parenthesized: an ordinary string expression statement,
// never a directive
('use strict');
