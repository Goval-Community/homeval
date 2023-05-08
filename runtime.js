// rome-ignore lint/suspicious/noShadowRestrictedNames: <explanation>
((globalThis) => {
	const core = Deno.core;

	function argsToMessage(...args) {
		return args.map((arg) => JSON.stringify(arg)).join(" ");
	}

	core.initializeAsyncOps();

	globalThis.console = {
		log: (...args) => {
			core.print(`[out]: ${argsToMessage(...args)}\n`, false);
		},
		error: (...args) => {
			core.print(`[err]: ${argsToMessage(...args)}\n`, true);
		},
	}; //
})(globalThis);
