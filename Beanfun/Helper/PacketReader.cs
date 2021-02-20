using System;
using System.IO;
using System.Text;

namespace MapleLib.PacketLib
{
	/// <summary>
	/// Class to handle reading data from a packet
	/// </summary>
	public class PacketReader : IDisposable
	{
		protected MemoryStream _buffer;

		/// <summary>
		/// The main reader tool
		/// </summary>
		private readonly BinaryReader _binReader;

		/// <summary>
		/// Amount of data left in the reader
		/// </summary>
		public int Length
		{
			get { return (int)_buffer.Length; }
		}

		/// <summary>
		/// Creates a new instance of PacketReader
		/// </summary>
		/// <param name="arrayOfBytes">Starting byte array</param>
		public PacketReader(byte[] arrayOfBytes)
		{
			_buffer = new MemoryStream(arrayOfBytes, false);
			_binReader = new BinaryReader(_buffer, Encoding.Default);
		}

		bool disposed = false;
		public void Dispose()
		{
			if (disposed) return;
			_binReader.Close();
			_buffer.Dispose();

			disposed = true;
		}

		/// <summary>
		/// Restart reading from the point specified.
		/// </summary>
		/// <param name="length">The point of the packet to start reading from.</param>
		public void Reset(int length)
		{
			_buffer.Seek(length, SeekOrigin.Begin);
		}

		public void Skip(int length)
		{
			_buffer.Position += length;
		}

		public int Position
		{
			get
			{
				return (int)_buffer.Position;
			}
		}
		public int Remaining
		{
			get
			{
				return Length - (int)_buffer.Position;
			}
		}

		/// <summary>
		/// Reads an unsigned byte from the stream
		/// </summary>
		/// <returns> an unsigned byte from the stream</returns>
		public byte ReadByte()
		{
			return _binReader.ReadByte();
		}

		/// <summary>
		/// Reads a byte array from the stream
		/// </summary>
		/// <param name="length">Amount of bytes</param>
		/// <returns>A byte array</returns>
		public byte[] ReadBytes(int count)
		{
			return _binReader.ReadBytes(count);
		}

		/// <summary>
		/// Reads a bool from the stream
		/// </summary>
		/// <returns>A bool</returns>
		public bool ReadBool()
		{
			return _binReader.ReadBoolean();
		}

		/// <summary>
		/// Reads a signed short from the stream
		/// </summary>
		/// <returns>A signed short</returns>
		public short ReadShort()
		{
			return _binReader.ReadInt16();
		}

		/// <summary>
		/// Reads an unsigned short from the stream
		/// </summary>
		/// <returns>A signed short</returns>
		public ushort ReadUShort()
		{
			return _binReader.ReadUInt16();
		}

		/// <summary>
		/// Reads a signed int from the stream
		/// </summary>
		/// <returns>A signed int</returns>
		public int ReadInt()
		{
			return _binReader.ReadInt32();
		}

		/// <summary>
		/// Reads an unsigned int from the stream
		/// </summary>
		/// <returns>A signed int</returns>
		public uint ReadUInt()
		{
			return _binReader.ReadUInt32();
		}

		/// <summary>
		/// Reads a signed long from the stream
		/// </summary>
		/// <returns>A signed long</returns>
		public long ReadLong()
		{
			return _binReader.ReadInt64();
		}

		/// <summary>
		/// Reads an unsigned long from the stream
		/// </summary>
		/// <returns>A signed long</returns>
		public ulong ReadULong()
		{
			return _binReader.ReadUInt64();
		}

		/// <summary>
		/// Reads an ASCII string from the stream
		/// </summary>
		/// <param name="length">Amount of bytes</param>
		/// <returns>An ASCII string</returns>
		public string ReadString(int length)
		{
			return Encoding.Default.GetString(ReadBytes(length));
		}

		/// <summary>
		/// Reads a maple string from the stream
		/// </summary>
		/// <returns>A maple string</returns>
		public string ReadMapleString()
		{
			return ReadString(ReadShort());
		}

		public byte[] ToArray()
		{
			return _buffer.ToArray();
		}
	}
}