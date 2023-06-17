class Service extends ServiceBase {
    constructor(...args) {
        super(...args)
        this.users = []
        this.files = {}

        this.session_map = {}
    }

	async recv(cmd, session) {
		if (cmd.followUser) {
            await this.send(api.Command.create({followUser: {session}}), cmd.followUser.session)
        } else if (cmd.unfollowUser) {
            await this.send(api.Command.create({unfollowUser: {session}}), cmd.followUser.session)
        } else if (cmd.openFile) {
            const user = this.session_map[session];
            
            const msg = {
                userId: user.id,
                session,
                timestamp: {
                    seconds: (Date.now()/ 1000n).toString(),
					nanos: 0
                }
            };

            if (cmd.openFile.file) {
                msg.file = cmd.openFile.file
            }

            this.files[session] = msg

            await this.send(api.Command.create({fileOpened: msg}), -session)
        } else {
            console.debug("Unknown presence command:", cmd);
        }
	}

    async attach(session) {
        const roster = api.Command.create({
            roster: {
                user: this.users,
                files: Object.values(this.files)
            }
        })

        await this.send(roster, session)
        
        const _user = await process.getUserInfo(session);

        this.session_map[session] = _user;
        
        const user = {
            id: _user.id,
            name: _user.username,
            session: session
        }

        this.users.push(user)

        await this.send(api.Command.create({ join: user }), -session)
    }

    async detach(session) {
        const user = this.session_map[session];
        delete this.files[session];

        await this.senc(api.Command.create({
            part: {
                id: user.id,
                name: user.username,
                session: session
            }
        }), -session)
    }
}

const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
