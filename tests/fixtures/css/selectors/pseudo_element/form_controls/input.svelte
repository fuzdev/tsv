<style>
	/* File input button */
	input[type='file']::file-selector-button {
		background: blue;
		padding: 10px;
	}

	/* Range slider parts */
	input[type='range']::slider-thumb {
		width: 20px;
		height: 20px;
	}

	input[type='range']::slider-track {
		background: gray;
	}

	/* Number input steppers */
	input[type='number']::step-up {
		cursor: pointer;
	}

	/* Form Control Icons */
	input[type='date']::picker-icon {
		color: blue;
	}

	input[type='checkbox']::checkmark {
		color: green;
	}

	input[type='color']::color-swatch {
		border: 1px solid gray;
	}

	input[type='search']::clear-icon {
		cursor: pointer;
	}

	input[type='password']::reveal-icon {
		cursor: pointer;
	}

	/* Form Field Components */
	input[type='datetime-local']::field-component {
		font-variant-numeric: tabular-nums;
	}

	input[type='datetime-local']::field-separator {
		color: gray;
	}

	input[type='datetime-local']::field-text {
		font-weight: bold;
	}
</style>
