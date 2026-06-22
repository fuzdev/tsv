<script>
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = someCondition || {
		fallbackKey: fallbackValue,
		another: 1
	};

	const bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb = someCondition && [
		fallbackValue,
		anotherValue,
		thirdValue
	];

	const ccccccccccccccccccccccccccccccccc = someCondition ?? {
		fallback: true,
		another: false,
		third: 'value'
	};

	const ddddddddddddddddddddddddddddddddd = someCondition || {};

	const eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee = someCondition || [];

	const fffffffffffffffffffffffffffffffffffffffffff = someCondition + someOtherValue;

	const x = foo || { a: 1 };

	const ggggggggggggggggggggggggggggggggg = a || b || { key: 'longvalue', another: 'longvalue2' };
</script>
