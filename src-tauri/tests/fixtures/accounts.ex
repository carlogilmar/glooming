defmodule MyApp.Accounts do
  @moduledoc """
  Reads and writes for the `users` table.
  """

  import Ecto.Changeset
  alias MyApp.{Repo, User}

  @required [:email, :name]

  @doc "Creates a user from raw attrs."
  def create_user(attrs) do
    attrs
    |> normalize()
    |> then(&changeset(%User{}, &1))
    |> Repo.insert()
  end

  @doc "Fetches a user, or nil."
  def get_user(id) do
    Repo.get(User, id)
  end

  def get_user!(id) do
    Repo.get!(User, id)
  end

  # -- private ---------------------------------------------------------

  defp normalize(%{email: email} = attrs) do
    %{attrs | email: email |> String.trim() |> String.downcase()}
  end

  defp normalize(attrs), do: attrs

  defp changeset(user, attrs) do
    user
    |> cast(attrs, @required)
    |> validate_required(@required)
    |> unique_constraint(:email)
  end
end
