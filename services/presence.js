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

        await this.send(roster, session)
        
        const _user = process.getUserInfo(session);
        
        const user = {
            id: _user.id,
            name: _user.username,
            session: session
        }

        this.users.push(user)

        await this.send(api.Command.create({ join: user }), -session)
    }
}

console.log(serviceInfo);
const service = new Service(
	serviceInfo.id,
	serviceInfo.service,
	serviceInfo.name,
);
await service.start()
