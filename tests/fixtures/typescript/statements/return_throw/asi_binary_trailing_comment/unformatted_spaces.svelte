<script>
	function   fn1()   {
		return   a + b   /* c1 */
	}


	const   obj   =   {
		get x()   {
			return   a && b   // c2
		},
		m()   {
			throw   a + b   /* c3 */
		}
	};


	const   fn2   =   ()   =>   {
		return   a || b   /* c4 */
	};
</script>
