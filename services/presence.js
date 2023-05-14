class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.users = []
        this.files = []
    }

	async recv(cmd, session) {
		console.log(cmd)
	}

    async attach(session) {
        const roster = api.Command.create({
            roster: {
                user: this.users,
                files: this.files
            }
        })

        await this._send(roster, session)
        
        const _user = Deno.core.ops.op_user_info(session);
        
        const user = {
            id: _user.id,
            name: _user.username,
            session: session
        }

        this.users.push(user)

        const msg = api.Command.create({
            join: user,
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
await service.start()
