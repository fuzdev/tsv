<script lang="ts">
	const obj = {content:data.prop === 'some_value' && data.fallback? to_some_long_formatter_result(data.fallback) || '': data.content};

	const obj2 = {
		value:condition?consequent:alternate,
	};

	const obj3 = {long_property_name_that_pushes_width: some_condition? long_consequent_expression_here: long_alternate_expression_here};

	const obj4 = {a: 1,b:very_long_test_condition && another_part_of_condition? very_long_consequent_value: very_long_alternate_value,c: 3};

	// Test || operator
	const obj5 = {prop:some_condition_value || some_fallback_value? consequent_for_this_case: alternate_for_this_case};

	// Test ?? operator (nullish coalescing)
	const obj6 = {prop:(some_value_may_be_null ?? some_fallback_default)? consequent_result_here: alternate_result_here};
</script>
