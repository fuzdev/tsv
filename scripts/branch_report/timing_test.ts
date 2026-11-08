import { deepStrictEqual, throws } from 'node:assert';

import { row_verdict } from './report.ts';
import { assert_instruction_samples, median, measured_rows } from './timing.ts';

Deno.test('median of no samples is null, never NaN', () => {
	deepStrictEqual(median([]), null);
	deepStrictEqual(median([3, 1, 2]), 2);
	deepStrictEqual(median([4, 1, 3, 2]), 2.5);
});

Deno.test('a one-sided measured row reads added or removed', () => {
	deepStrictEqual(row_verdict(measured_rows('wall', 'ms', [[], [5]]), undefined), 'added');
	deepStrictEqual(row_verdict(measured_rows('wall', 'ms', [[5], []]), undefined), 'removed');
});

Deno.test('a side with no instruction samples is refused', () => {
	assert_instruction_samples([[1], [2]], 'x');
	throws(() => assert_instruction_samples([[], [2]], 'x'), /no instructions on the base side/);
	throws(() => assert_instruction_samples([[1], []], 'x'), /no instructions on the candidate side/);
});
