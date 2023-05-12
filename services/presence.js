class Service extends ServiceBase {
	async recv(cmd, session) {
		console.log(cmd)
	}

    async attach(session) {
        const msg = api.Command.create({
            join: {
              id: 14536118,
              name: "PotentialStyx",
              session: session
            },
            session: -session
        })

        for (const arr_session of this.clients) {
            if (arr_session === session) {continue}
            await this._send(msg, arr_session)
        }
    }

    async _send(cmd, session) {
		cmd.channel = this.id;
		const buf = [...Buffer.from(api.Command.encode(cmd).finish())];
		await Deno.core.ops.op_send_msg({
			bytes: buf,
			session: session,
		});
	}
}

console.log(serviceInfo);
const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service._recv();
