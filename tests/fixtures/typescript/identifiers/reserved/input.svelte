<script lang="ts">
	// Reserved words allowed in property positions (ES5+)
	const obj = {
		class: 1,
		default() {
			return 2;
		},
		get if() {
			return 3;
		},
		set if(v) {
			this._if = v;
		},
	};
	const {class: c, default: d, if: i} = obj;
	const a = obj.class;
	const b = obj.default();
	const e = obj.if;
	obj.if = 4;
</script>
