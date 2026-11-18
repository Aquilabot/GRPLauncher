// GRP launcher: signs a player in to our account service, then starts the unmodified Ghost Recon Phantoms client
// against our servers and answers what the game asks its launcher over \\.\pipe\Ghost_Recon_Phantoms.
// No game file is changed and nothing of the game is shipped with this program.
//
//   GRPLauncher.exe                                   the window
//   GRPLauncher.exe --user N --password P [--invite C] [--game <GhostReconPhantoms.exe>]   no window (tests)
//
// Settings next to the program, launcher.ini: server, accountport, port, accesskey (see Config).
// What the player typed last (name and game folder, never the password): %APPDATA%\GRPLauncher\settings.txt
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Net;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Windows.Forms;

static class Config
{
    public static string Server = "56.127.0.17";
    public static int AccountPort = 21080;      // TCP, account service
    public static int Port = 21031;             // UDP, lobby
    public static string AccessKey = "8dtRv2oj";
    public static string Language = "english";

    public static string DataDir { get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "GRPLauncher"); } }
    public static string DefaultGame { get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Phoenix Network", "Ghost Recon Phantoms", "GhostReconPhantoms.exe"); } }

    public static Dictionary<string, string> ReadPairs(string file)
    {
        var d = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        if (!File.Exists(file)) return d;
        foreach (string line in File.ReadAllLines(file))
        {
            int eq = line.IndexOf('=');
            if (eq > 0 && !line.TrimStart().StartsWith("#")) d[line.Substring(0, eq).Trim()] = line.Substring(eq + 1).Trim();
        }
        return d;
    }

    public static void Load()
    {
        var d = ReadPairs(Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "launcher.ini"));
        string v; int n;
        if (d.TryGetValue("server", out v) && v != "") Server = v;
        if (d.TryGetValue("accountport", out v) && int.TryParse(v, out n)) AccountPort = n;
        if (d.TryGetValue("port", out v) && int.TryParse(v, out n)) Port = n;
        if (d.TryGetValue("accesskey", out v) && v != "") AccessKey = v;
        if (d.TryGetValue("language", out v) && v != "") Language = v;
    }
}

static class Account
{
    // What goes to the server instead of the password: PBKDF2-HMAC-SHA1(password, "grp2016:" + lowercase name),
    // 50000 rounds, 32 bytes, in hex. The server stores it salted and hashed again.
    public static string Secret(string name, string password)
    {
        using (var k = new Rfc2898DeriveBytes(Encoding.UTF8.GetBytes(password), Encoding.UTF8.GetBytes("grp2016:" + name.ToLowerInvariant()), 50000))
            return string.Concat(k.GetBytes(32).Select(b => b.ToString("x2")));
    }

    // One line back from the account service: "OK ..." or "ERR <reason>"; a network failure comes back as "NET <text>".
    public static string Post(string path, string form)
    {
        try
        {
            var req = (HttpWebRequest)WebRequest.Create("http://" + Config.Server + ":" + Config.AccountPort + path);
            req.Method = "POST"; req.ContentType = "application/x-www-form-urlencoded"; req.Timeout = 10000; req.Proxy = null;
            byte[] body = Encoding.ASCII.GetBytes(form);
            req.ContentLength = body.Length;
            using (var s = req.GetRequestStream()) s.Write(body, 0, body.Length);
            using (var resp = (HttpWebResponse)req.GetResponse())
            using (var r = new StreamReader(resp.GetResponseStream())) return r.ReadToEnd().Trim();
        }
        catch (WebException ex)
        {
            if (ex.Response != null)
                using (var r = new StreamReader(ex.Response.GetResponseStream())) return r.ReadToEnd().Trim();
            return "NET " + ex.Message;
        }
        catch (Exception ex) { return "NET " + ex.Message; }
    }

    public static string Register(string invite, string name, string password)
    {
        return Post("/register", "invite=" + Uri.EscapeDataString(invite) + "&name=" + Uri.EscapeDataString(name) + "&secret=" + Secret(name, password));
    }

    public static string Login(string name, string password)
    {
        return Post("/login", "name=" + Uri.EscapeDataString(name) + "&secret=" + Secret(name, password));
    }

    public static string Explain(string answer)
    {
        switch (answer)
        {
            case "ERR invite": return "El código de invitación no es válido o ya se usó.";
            case "ERR name": return "El nickname debe tener de 3 a 16 caracteres: letras, números, guion y guion bajo.";
            case "ERR taken": return "Ese nickname ya está en uso.";
            case "ERR login": return "Nickname o contraseña incorrectos.";
            case "ERR slow": return "Demasiados intentos. Espera un minuto.";
            case "ERR secret": case "ERR size": case "ERR path": case "ERR template": return "El servidor rechazó la solicitud (" + answer + ").";
        }
        if (answer.StartsWith("NET ")) return "No se pudo conectar con el servidor: " + answer.Substring(4);
        return "Respuesta inesperada del servidor: " + answer;
    }
}

// The pipe the game's own launcher protocol runs on. Messages are a sequence of [u32 byte length][UTF-16LE text];
// the game sends ["v2"][command]. Answers are binary: a status byte 01 followed by the value.
static class GamePipe
{
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr CreateNamedPipeW(string name, uint openMode, uint pipeMode, uint maxInstances, uint outSize, uint inSize, uint timeout, IntPtr sa);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool ConnectNamedPipe(IntPtr h, IntPtr ov);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool DisconnectNamedPipe(IntPtr h);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool ReadFile(IntPtr h, byte[] buf, uint len, out uint read, IntPtr ov);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool WriteFile(IntPtr h, byte[] buf, uint len, out uint written, IntPtr ov);
    [DllImport("kernel32.dll", SetLastError = true)] static extern bool CloseHandle(IntPtr h);
    const uint PIPE_ACCESS_DUPLEX = 3, PIPE_TYPE_MESSAGE = 4, PIPE_READMODE_MESSAGE = 2;

    static string user = "";
    static bool started;

    public static void Start(string gameUser)
    {
        user = gameUser;
        if (started) return;
        started = true;
        // The game fires several requests back to back, each on a new connection: several instances listen.
        for (int i = 0; i < 4; i++) new Thread(Serve) { IsBackground = true }.Start();
    }

    static void Serve()
    {
        while (true)
        {
            IntPtr h = CreateNamedPipeW(@"\\.\pipe\Ghost_Recon_Phantoms", PIPE_ACCESS_DUPLEX, PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE, 255, 4096, 4096, 0, IntPtr.Zero);
            if (h == new IntPtr(-1)) { Program.Log("pipe: create failed " + Marshal.GetLastWin32Error()); Thread.Sleep(1000); continue; }
            if (!ConnectNamedPipe(h, IntPtr.Zero) && Marshal.GetLastWin32Error() != 535) { CloseHandle(h); continue; }
            var buf = new byte[4096];
            uint r, w;
            while (ReadFile(h, buf, (uint)buf.Length, out r, IntPtr.Zero) && r > 0)
            {
                var parts = Unframe(buf.Take((int)r).ToArray());
                Program.Log("pipe: " + string.Join(" ", parts));
                byte[] reply = Reply(parts);
                if (reply != null) WriteFile(h, reply, (uint)reply.Length, out w, IntPtr.Zero);
            }
            DisconnectNamedPipe(h);
            CloseHandle(h);
        }
    }

    static List<string> Unframe(byte[] m)
    {
        var parts = new List<string>();
        int o = 0;
        while (o + 4 <= m.Length)
        {
            int len = BitConverter.ToInt32(m, o); o += 4;
            if (len < 0 || o + len > m.Length) break;
            parts.Add(Encoding.Unicode.GetString(m, o, len)); o += len;
        }
        return parts;
    }

    static byte[] Frame(params string[] parts)
    {
        var ms = new MemoryStream();
        foreach (var p in parts)
        {
            var b = Encoding.Unicode.GetBytes(p);
            ms.Write(BitConverter.GetBytes(b.Length), 0, 4);
            ms.Write(b, 0, b.Length);
        }
        return ms.ToArray();
    }

    static byte[] Cat(params byte[][] parts) { return parts.SelectMany(p => p).ToArray(); }

    static byte[] Reply(List<string> request)
    {
        if (request.Count < 2) return null;
        byte[] ok = { 1 };
        switch (request[1])
        {
            case "epochnow": return Cat(ok, BitConverter.GetBytes((long)(DateTime.UtcNow - new DateTime(1970, 1, 1)).TotalSeconds));
            case "validate": return ok;
            case "notifyclientstart": return Frame("ok");
            case "showeula": return new byte[] { 1, 0 };
            case "getdistributionsource": return Cat(ok, Frame(""));
            case "getlogininfo": return Cat(ok, Frame(user, "grp"));   // the server checks the token inside the user string
            case "getproxy": return new byte[] { 1, 0, 0, 0, 0, 0 };
            default: return ok;
        }
    }
}

static class Program
{
    static readonly object logLock = new object();
    public static void Log(string s)
    {
        try
        {
            Directory.CreateDirectory(Config.DataDir);
            lock (logLock) File.AppendAllText(Path.Combine(Config.DataDir, "launcher.log"), DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss.fff") + " " + s + Environment.NewLine);
        }
        catch { }
    }

    // Signs in and starts the game. Returns null when the game ran and exited, or the message for the player.
    public static string Play(string name, string password, string gameExe, Action<string> status)
    {
        if (!File.Exists(gameExe)) return "No se encontró el juego en:\n" + gameExe;
        status("Iniciando sesión...");
        string answer = Account.Login(name, password);
        string[] a = answer.Split(' ');
        if (a.Length != 3 || a[0] != "OK") { Log("login refused: " + (answer.StartsWith("OK") ? "bad answer" : answer)); return Account.Explain(answer); }
        Log("signed in as " + a[1]);
        GamePipe.Start(a[1] + "." + a[2]);
        Thread.Sleep(500);
        status("Abriendo el juego...");
        var psi = new ProcessStartInfo(gameExe, "/ForceDefaultSandbox=true /Sandbox=" + Config.Server + " /Port=" + Config.Port + " /OnlineAccessKey=" + Config.AccessKey + " /language=" + Config.Language)
        {
            UseShellExecute = false,
            WorkingDirectory = Path.GetDirectoryName(gameExe)
        };
        Process game;
        try { game = Process.Start(psi); }
        catch (Exception ex) { Log("game start failed: " + ex.Message); return "No se pudo abrir el juego: " + ex.Message; }
        Log("game started, pid " + game.Id);
        status("Jugando. Esta ventana vuelve cuando cierres el juego.");
        game.WaitForExit();
        Log("game exited, code " + game.ExitCode);
        return null;
    }

    static string Arg(string[] args, string key)
    {
        int i = Array.IndexOf(args, key);
        return i >= 0 && i + 1 < args.Length ? args[i + 1] : null;
    }

    [STAThread]
    static int Main(string[] args)
    {
        Config.Load();
        Log("launcher started, server " + Config.Server);
        if (Arg(args, "--user") != null)
        {
            string name = Arg(args, "--user"), pw = Arg(args, "--password") ?? "", invite = Arg(args, "--invite");
            if (invite != null)
            {
                string r = Account.Register(invite, name, pw);
                Console.WriteLine("register: " + r);
                if (r != "OK") return 2;
            }
            string err = Play(name, pw, Arg(args, "--game") ?? Config.DefaultGame, s => Console.WriteLine(s));
            if (err != null) { Console.WriteLine(err); return 1; }
            return 0;
        }
        Application.EnableVisualStyles();
        Application.Run(new LauncherForm());
        return 0;
    }
}

class LauncherForm : Form
{
    readonly TextBox name = new TextBox(), password = new TextBox(), invite = new TextBox(), game = new TextBox();
    readonly Button play = new Button(), browse = new Button();
    readonly CheckBox create = new CheckBox();
    readonly Label status = new Label(), inviteLabel = new Label();
    readonly string settingsFile = Path.Combine(Config.DataDir, "settings.txt");

    Label Caption(string text, int y)
    {
        var l = new Label { Text = text, Left = 24, Top = y, Width = 372, ForeColor = Color.FromArgb(150, 170, 180) };
        Controls.Add(l);
        return l;
    }

    void Field(TextBox t, int y, int width = 372)
    {
        t.Left = 24; t.Top = y; t.Width = width;
        t.BackColor = Color.FromArgb(30, 38, 44); t.ForeColor = Color.White; t.BorderStyle = BorderStyle.FixedSingle;
        Controls.Add(t);
    }

    public LauncherForm()
    {
        Text = "GRP Launcher";
        ClientSize = new Size(420, 404);
        FormBorderStyle = FormBorderStyle.FixedSingle; MaximizeBox = false; StartPosition = FormStartPosition.CenterScreen;
        BackColor = Color.FromArgb(18, 24, 28); ForeColor = Color.White; Font = new Font("Segoe UI", 10f);

        Controls.Add(new Label { Text = "GHOST RECON PHANTOMS", Left = 24, Top = 18, Width = 372, Height = 28, Font = new Font("Segoe UI", 14f, FontStyle.Bold), ForeColor = Color.FromArgb(120, 220, 230) });
        Caption("Nickname", 58); Field(name, 80);
        Caption("Contraseña", 114); Field(password, 136); password.UseSystemPasswordChar = true;

        create.Text = "Crear cuenta nueva"; create.Left = 24; create.Top = 172; create.Width = 372; create.ForeColor = Color.FromArgb(200, 210, 215);
        create.CheckedChanged += (s, e) => { invite.Visible = inviteLabel.Visible = create.Checked; play.Text = create.Checked ? "CREAR CUENTA Y JUGAR" : "JUGAR"; };
        Controls.Add(create);
        inviteLabel.Text = "Código de invitación"; inviteLabel.Left = 24; inviteLabel.Top = 200; inviteLabel.Width = 372; inviteLabel.ForeColor = Color.FromArgb(150, 170, 180); inviteLabel.Visible = false;
        Controls.Add(inviteLabel);
        Field(invite, 222); invite.Visible = false;

        Caption("Juego (GhostReconPhantoms.exe)", 258); Field(game, 280, 282);
        browse.Text = "Buscar"; browse.Left = 314; browse.Top = 279; browse.Width = 82; browse.Height = 27; browse.FlatStyle = FlatStyle.Flat; browse.ForeColor = Color.White;
        browse.Click += (s, e) =>
        {
            using (var d = new OpenFileDialog { Filter = "GhostReconPhantoms.exe|GhostReconPhantoms.exe|Programas|*.exe", Title = "Dónde está el juego" })
                if (d.ShowDialog(this) == DialogResult.OK) game.Text = d.FileName;
        };
        Controls.Add(browse);

        play.Text = "JUGAR"; play.Left = 24; play.Top = 322; play.Width = 372; play.Height = 40; play.FlatStyle = FlatStyle.Flat;
        play.BackColor = Color.FromArgb(70, 140, 110); play.ForeColor = Color.White; play.Font = new Font("Segoe UI", 11f, FontStyle.Bold);
        play.Click += (s, e) => Go();
        Controls.Add(play);
        AcceptButton = play;

        status.Left = 24; status.Top = 368; status.Width = 372; status.Height = 32; status.ForeColor = Color.FromArgb(230, 190, 110); status.Font = new Font("Segoe UI", 8.5f);
        Controls.Add(status);

        var saved = Config.ReadPairs(settingsFile);
        string v;
        name.Text = saved.TryGetValue("name", out v) ? v : "";
        game.Text = saved.TryGetValue("game", out v) && v != "" ? v : Config.DefaultGame;
        Shown += (s, e) => { if (name.Text == "") name.Focus(); else password.Focus(); };
    }

    void SetStatus(string s) { if (IsHandleCreated) BeginInvoke((Action)(() => status.Text = s)); }

    void Go()
    {
        string n = name.Text.Trim(), p = password.Text, inv = invite.Text.Trim(), g = game.Text.Trim();
        bool register = create.Checked;
        if (n == "" || p == "") { status.Text = "Escribe tu nickname y tu contraseña."; return; }
        if (register && p.Length < 6) { status.Text = "La contraseña debe tener al menos 6 caracteres."; return; }
        if (register && inv == "") { status.Text = "Escribe el código de invitación."; return; }
        try { Directory.CreateDirectory(Config.DataDir); File.WriteAllLines(settingsFile, new[] { "name=" + n, "game=" + g }); } catch { }
        play.Enabled = false;
        new Thread(() =>
        {
            string err = null;
            if (register)
            {
                SetStatus("Creando la cuenta...");
                string r = Account.Register(inv, n, p);
                Program.Log("register " + n + ": " + r);
                if (r != "OK") err = Account.Explain(r);
            }
            if (err == null)
            {
                if (register) BeginInvoke((Action)(() => create.Checked = false));
                BeginInvoke((Action)(() => WindowState = FormWindowState.Normal));
                err = Program.Play(n, p, g, s => { SetStatus(s); if (s.StartsWith("Jugando")) BeginInvoke((Action)(() => WindowState = FormWindowState.Minimized)); });
            }
            BeginInvoke((Action)(() =>
            {
                WindowState = FormWindowState.Normal;
                status.Text = err ?? "";
                play.Enabled = true;
                password.Text = "";
            }));
        }) { IsBackground = true }.Start();
    }
}
