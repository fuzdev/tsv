<script lang="ts">
	// A glued pair before the first type param stays glued; the param breaks below.
	function fn1<
		/* c1 */ /* c2 */
		TFirstLongTypeParameter extends string = string,
		TSecondLongTypeParameter extends number = number
	>(x: TFirstLongTypeParameter) {}

	// A glued pair between params stays glued.
	function fn2<
		TFirstLongTypeParameter extends string = string,
		/* c1 */ /* c2 */
		TSecondLongTypeParameter extends number = number
	>(x: TFirstLongTypeParameter) {}

	// A glued pair after a line comment stays glued.
	function fn3<
		TFirstLongTypeParameter extends string = string,
		// line
		/* c1 */ /* c2 */
		TSecondLongTypeParameter extends number = number
	>(x: TFirstLongTypeParameter) {}

	// Blocks the author put on their own lines keep them.
	function fn4<
		TFirstLongTypeParameter extends string = string,
		/* c1 */
		/* c2 */
		TSecondLongTypeParameter extends number = number
	>(x: TFirstLongTypeParameter) {}

	// A class type-param declaration takes the wrapping router.
	class Single<
		/* c1 */ /* c2 */
		TFirstLongTypeParameter extends string = string,
		TSecondLongTypeParameter extends number = number
	> {}

	// An interface type-param declaration.
	interface Multiple<
		/* c1 */ /* c2 */
		TFirstLongTypeParameter extends string = string,
		TSecondLongTypeParameter extends number = number
	> {}
</script>
