<script lang="ts">
	// Union types

	// 100 chars exactly - stays inline (at boundary)
	type A = string | number | boolean | object | null | undefined | symbol | bigint | neveeeeeeeeeer;

	// 101 chars - wraps with leading |
	type B =
		| string
		| number
		| boolean
		| object
		| null
		| undefined
		| symbol
		| bigint
		| neveeeeeeeeeeer;

	// Intersection types

	// 100 chars exactly - stays inline (at boundary)
	type C = string & number & boolean & object & null & undefined & symbol & bigint & neverrrrrrrrrr;

	// 101 chars - wraps with trailing &
	type D = string &
		number &
		boolean &
		object &
		null &
		undefined &
		symbol &
		bigint &
		neverrrrrrrrrrr;
</script>
