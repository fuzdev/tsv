// an authored-expanded type parameter default refuses the hug, whatever the parameters
fn(
	<
		T = {
			a: 1;
		}
	>(
		a
	) => {
		g(a);
	}
);
