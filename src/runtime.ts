// @ts-nocheck
// This file is outdated see `src/runtime.js` and `types/homeval-types/types.d.ts`

import { api } from "@replit/protocol";
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
	};
})(globalThis);

interface ServiceInterface {
	// rome-ignore lint/suspicious/noExplicitAny: services can have any data that they want
	data: any;
	clients: number[];
	// internal id used for api stuff
	internal: string;
	id: number;
	service: string;
	name?: string;

	recv(message: api.Command, session: number): api.Command | undefined;

	send(message: api.Command, sessions: number[]);

	attach(session: number);
}

class ServiceBase implements ServiceInterface {
	// rome-ignore lint/suspicious/noExplicitAny: services can have any data that they want
	public data: any;
	public clients: number[];
	constructor(
		public internal: string,
		public id: number,
		public service: string,
		public name?: string,
	) {
		this.clients = [];
	}

	public recv(_c: api.Command, _s: number): undefined {
		throw new Error("Not implemented");
	}

	public send(_c: api.Command, _s: number[]) {
		throw new Error("Not implemented");
	}

	public attach(session: number) {
		this.clients.push(session);
	}
}
