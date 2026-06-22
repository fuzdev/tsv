<script lang="ts">
	// 100 chars - no wrap
	const obj1={prop_aaaaaaaaaa:aaaa.bbbb==null?cccc:dddd.fn(new eeee(aaaa.fffffffffffffffffff).method()),};

	// 101 chars with prop, ternary alone 85 chars - wraps after colon, ternary intact
	const obj2={prop_aaaaaaaaaaa:aaaa.bbbb==null?cccc:dddd.fn(new eeee(aaaa.fffffffffffffffffff).method()),};

	// 101 chars at deeper indent - wraps after colon, then ternary breaks ? :
	const obj3={value:aaaa.bbbb==null?cccc:dddd.fn(new eeee(aaaa.fffffffffffffffffffffffffffffffffff).method()),};

	// 101 chars on alternate branch - wraps function args
	const obj4={value:aaaa.bbbb==null?cccc:dddd.fn(new eeeeeeeeeeeeeeeeee(aaaaaaaaaaa.fffffffffffffffffffffffffffffffffffff).method()),};

	// 101 chars on new expression - wraps constructor args
	const obj5={value:aaaa.bbbb==null?cccc:dddd.fn(new eeeeeeeeeeeeeeeeee(aaaaaaaaaaa.ffffffffffffffffffffffffffffffffffffffffffff).method()),};
</script>
